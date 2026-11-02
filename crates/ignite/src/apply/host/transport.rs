//! The host's edges: the exclusive state lock, the state identity, the
//! control socket with its framing, and the stdin and signal controls.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::Duration;

use ignite_eval::Json;
use nix::fcntl::{Flock, FlockArg};

use super::{Host, Message, Reply};
use crate::apply::{Failure, PROTOCOL, fail, object, read_frame, required, text, write_frame};
use crate::run::control::Observation;

/// Hold the state directory against a second host or run.
pub(super) fn lock(root: &Path) -> Result<Flock<File>, Failure> {
    let lock_file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(root.join("lock"))?;
    Flock::lock(lock_file, FlockArg::LockExclusiveNonblock).map_err(|(_, error)| {
        fail(
            3,
            format!("state is locked by another host or run: {error}"),
        )
    })
}

/// The state identity, created on first start.
pub(super) fn identity(root: &Path) -> Result<String, Failure> {
    let identity_path = root.join("identity");
    Ok(if identity_path.exists() {
        std::fs::read_to_string(&identity_path)?
    } else {
        let mut random = [0; 32];
        File::open("/dev/urandom")?.read_exact(&mut random)?;
        let id = crate::apply::digest::bytes(&random);
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&identity_path)?;
        file.write_all(id.as_bytes())?;
        file.sync_all()?;
        File::open(root)?.sync_all()?;
        id
    })
}

/// Bind the control socket and accept clients until the returned flag is
/// set; each connection carries one request frame and one response frame.
pub(super) fn listen(
    root: &Path,
    tx: mpsc::Sender<Message>,
) -> Result<(PathBuf, Arc<AtomicBool>), Failure> {
    let socket_path = root.join("control.sock");
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }
    let (_state_directory, socket_address) = crate::apply::socket_address(root)?;
    let listener = UnixListener::bind(socket_address)?;
    std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o600))?;
    listener.set_nonblocking(true)?;
    let stopping = Arc::new(AtomicBool::new(false));
    let accept_stop = Arc::clone(&stopping);
    std::thread::spawn(move || {
        while !accept_stop.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let tx = tx.clone();
                    std::thread::spawn(move || {
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(15)));
                        let result = read_frame(&mut stream);
                        let id = result
                            .as_ref()
                            .ok()
                            .and_then(|request| request.get("requestId"))
                            .and_then(Json::as_str)
                            .unwrap_or("")
                            .to_string();
                        let result = result.map_err(Failure::from).and_then(|request| {
                            let (reply, wait) = mpsc::channel();
                            tx.send(Message::Request(request, reply))
                                .map_err(|_| fail(3, "host queue closed"))?;
                            wait.recv()
                                .map_err(|_| fail(3, "host stopped before replying"))?
                        });
                        let (status, reason, payload) = match result {
                            Ok(value) => (0, String::new(), value),
                            Err(error) => (error.status, error.reason, Json::Null),
                        };
                        let response = object([
                            ("version", text(PROTOCOL)),
                            ("requestId", text(id)),
                            ("status", Json::Int(i64::from(status))),
                            ("reason", text(reason)),
                            ("payload", payload),
                        ]);
                        let _ = write_frame(&mut stream, &response);
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                Err(_) => break,
            }
        }
    });
    Ok((socket_path, stopping))
}

/// Forward `stop`, `status` and interrupts from stdin and signals.
pub(super) fn watch_control(host: mpsc::Sender<Message>) -> Result<(), Failure> {
    let (control_tx, control_rx) = mpsc::channel();
    crate::run::control::read_control(control_tx.clone());
    crate::run::control::watch_signals(control_tx)?;
    std::thread::spawn(move || {
        for observation in control_rx {
            match observation {
                Observation::Interrupt => {
                    let _ = host.send(Message::Stop);
                }
                Observation::Control(command) if command == "stop" => {
                    let _ = host.send(Message::Stop);
                }
                Observation::Control(command) if command == "status" => {
                    let _ = host.send(Message::Status);
                }
                _ => {}
            }
        }
    });
    Ok(())
}

impl Host {
    /// Check one request's envelope and route it; errors go to `reply`.
    pub(super) fn request(
        &mut self,
        request: &Json,
        reply: &Reply,
        shutdown: bool,
    ) -> Result<(), Failure> {
        if required(request, "version")? != PROTOCOL {
            return Err(fail(2, "unsupported control protocol"));
        }
        required(request, "requestId")?;
        let operation = required(request, "operation")?;
        let identity = required(request, "stateIdentity")?;
        if matches!(operation, "prepare" | "apply") && identity.is_empty() {
            return Err(fail(2, "request requires state identity"));
        }
        if !identity.is_empty() && identity != self.identity {
            return Err(fail(2, "request targets another host state"));
        }
        let payload = request.get("payload").cloned().unwrap_or(Json::Null);
        match operation {
            "snapshot" | "inspect" => {
                let mut snapshot = self.snapshot(true);
                if let Some(id) = payload.as_str() {
                    if !self.receipts.contains_key(id) {
                        return Err(fail(2, "unknown operation"));
                    }
                    if let Json::Object(fields) = &mut snapshot {
                        fields.insert("operation".into(), self.receipt(id));
                    }
                }
                let _ = reply.send(Ok(snapshot));
            }
            "prepare" if !shutdown => self.prepare(payload, reply.clone())?,
            "apply" if !shutdown => self.apply(payload, reply.clone())?,
            _ => return Err(fail(2, "unsupported operation or host is stopping")),
        }
        Ok(())
    }
}
