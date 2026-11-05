//! The host's edges: the exclusive state lock, the state identity, the
//! control socket with its framing, and the stdin and signal controls.

use std::fs::{DirBuilder, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
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
use crate::apply::{Failure, plan::Plan};
use crate::apply::{
    FrameError, Liveness, PROTOCOL, describe, envelope, fail, object, read_frame, text, write_frame,
};
use crate::run::control::Observation;

/// The control socket's name in the state directory.
const SOCKET: &str = "control.sock";
/// The private directory the socket is bound in before it is renamed to
/// `SOCKET`, so it is never reachable with wider permissions.
const SOCKET_STAGING: &str = "control.d";
/// The state identity's file name in the state directory.
const IDENTITY: &str = "identity";
/// The files whose presence means the state has host history, which is
/// bound to its identity.
const JOURNAL: &str = "host-journal.jsonl";
const STATUS: &str = "status.json";

/// Hold the state directory against a second host or run, then restrict
/// the directory to its owner. A state owned by another user, a state a
/// plain `ignite run` journals into, and a held lock are refused (exit 2)
/// before anything in the directory changes. A flock proves nothing once
/// the lock file was deleted or replaced, so the lock taken must be the
/// file at the path, and no host may still answer on `control.sock`.
pub(super) fn lock(root: &Path) -> Result<Flock<File>, Failure> {
    let metadata = std::fs::metadata(root)?;
    let user = std::fs::metadata("/proc/self")?.uid();
    if metadata.uid() != user {
        return Err(fail(
            2,
            format!(
                "state {} belongs to uid {}, not to this user (uid {user})",
                root.display(),
                metadata.uid()
            ),
        ));
    }
    let lock_path = root.join(crate::run::journal::LOCK);
    let lock_file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)?;
    let in_use = || {
        fail(
            2,
            format!(
                "state {} is in use by a running `ignite host` or `ignite run`; stop that \
                 process (stdin `stop` or SIGTERM) and retry. Its lock is held, not stale: \
                 deleting it does not release the state",
                root.display()
            ),
        )
    };
    let lock = match Flock::lock(lock_file, FlockArg::LockExclusiveNonblock) {
        Ok(lock) => lock,
        Err((_, nix::errno::Errno::EWOULDBLOCK)) => return Err(in_use()),
        Err((_, errno)) => return Err(fail(3, format!("{}: {errno}", lock_path.display()))),
    };
    let held = lock.metadata()?;
    let same_file = std::fs::symlink_metadata(&lock_path)
        .is_ok_and(|at_path| at_path.dev() == held.dev() && at_path.ino() == held.ino());
    if !same_file {
        return Err(fail(
            2,
            format!(
                "the lock file {} was replaced while this host took it; another process is \
                 using the state",
                lock_path.display()
            ),
        ));
    }
    if crate::apply::connect(root).is_ok() {
        return Err(in_use());
    }
    if root.join(crate::run::journal::FILE).exists() {
        return Err(fail(
            2,
            format!(
                "state {} holds an `ignite run` journal; a host needs its own state directory",
                root.display()
            ),
        ));
    }
    std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700))?;
    Ok(lock)
}

/// Whether `id` can be a state identity this host wrote: 1 to 128 bytes of
/// ASCII letters, digits, `:`, `-` and `_`.
fn valid_identity(id: &str) -> bool {
    (1..=128).contains(&id.len())
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b":-_".contains(&byte))
}

/// The state identity, created on first start: 32 random bytes in hex,
/// written to a temporary file, synced, renamed into place and the
/// directory synced, so a crash leaves either no identity or a whole one.
/// An identity file that is not one is a storage failure (exit 3), never
/// replaced.
pub(super) fn identity(root: &Path) -> Result<String, Failure> {
    let path = root.join(IDENTITY);
    match std::fs::read_to_string(&path) {
        Ok(id) if valid_identity(&id) => return Ok(id),
        Ok(id) => {
            return Err(fail(
                3,
                format!(
                    "{} is no state identity ({} bytes); restore it or use a new state",
                    path.display(),
                    id.len()
                ),
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(fail(3, format!("{}: {error}", path.display()))),
    }
    for history in [JOURNAL, STATUS] {
        if root.join(history).exists() {
            return Err(fail(
                3,
                format!(
                    "{} is missing but {} records host history bound to it; restore the \
                     identity file, a new identity would orphan that history",
                    path.display(),
                    root.join(history).display()
                ),
            ));
        }
    }
    let mut random = [0; 32];
    File::open("/dev/urandom")?.read_exact(&mut random)?;
    let id: String = std::iter::once("state:".to_string())
        .chain(random.iter().map(|byte| format!("{byte:02x}")))
        .collect();
    let temporary = root.join(format!(".{IDENTITY}.{}.writing", std::process::id()));
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temporary)?;
    file.write_all(id.as_bytes())?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&temporary, &path)?;
    File::open(root)?.sync_all()?;
    Ok(id)
}

/// Bind the control socket and accept clients until the returned flag is
/// set; each connection carries one request frame and one response frame.
/// The socket is bound inside a fresh owner-only directory, restricted to
/// the owner there, and only then renamed over `<state>/control.sock`: a
/// stale socket is replaced by the lock holder alone, in one step.
pub(super) fn listen(
    root: &Path,
    tx: mpsc::Sender<Message>,
) -> Result<(PathBuf, Arc<AtomicBool>), Failure> {
    let socket_path = root.join(SOCKET);
    let staging = root.join(SOCKET_STAGING);
    match std::fs::remove_dir_all(&staging) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(fail(3, format!("{}: {error}", staging.display()))),
    }
    DirBuilder::new().mode(0o700).create(&staging)?;
    std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o700))?;
    let listener = {
        let (_staging_directory, address) = crate::apply::socket_address(&staging)?;
        UnixListener::bind(address)?
    };
    std::fs::set_permissions(staging.join(SOCKET), std::fs::Permissions::from_mode(0o600))?;
    std::fs::rename(staging.join(SOCKET), &socket_path)?;
    std::fs::remove_dir(&staging)?;
    File::open(root)?.sync_all()?;
    listener.set_nonblocking(true)?;
    let stopping = Arc::new(AtomicBool::new(false));
    let accept_stop = Arc::clone(&stopping);
    std::thread::spawn(move || {
        while !accept_stop.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((stream, _)) => {
                    let tx = tx.clone();
                    std::thread::spawn(move || serve(stream, &tx));
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

/// Answer one connection. Frames that are oversized, not UTF-8 JSON, or
/// whose envelope fails `envelope` are answered with status 2 here, before
/// the host queue sees them; the reply carries the request id when one
/// could be read. A peer gone before a whole frame gets no reply.
fn serve(mut stream: UnixStream, tx: &mpsc::Sender<Message>) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(15)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(15)));
    let (id, result) = match read_frame(&mut stream) {
        Err(FrameError::Closed(_)) => return,
        Err(error) => (String::new(), Err(fail(2, error.reason()))),
        Ok(request) => match envelope(&request) {
            Err((id, failure)) => (id, Err(failure)),
            Ok(checked) => {
                let id = checked.id.to_string();
                let (reply, wait) = mpsc::channel();
                let result = tx
                    .send(Message::Request(request, reply))
                    .map_err(|_| fail(3, "host queue closed"))
                    .and_then(|()| {
                        wait.recv()
                            .map_err(|_| fail(3, "host stopped before replying"))?
                    });
                (id, result)
            }
        },
    };
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
    if let Err(error) = write_frame(&mut stream, &response) {
        eprintln!("ignite host: control reply not sent: {error}");
    }
    let _ = stream.shutdown(std::net::Shutdown::Write);
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
    /// The inspect view of this host (17.1): the snapshot with its live
    /// process, the readiness of the running revision and the next action.
    pub(super) fn inspection(&self) -> Json {
        let pid = std::process::id();
        let (mut expected, mut ready): (Vec<_>, Vec<_>) =
            self.runtime
                .as_ref()
                .map_or_else(Default::default, |runtime| {
                    (
                        runtime.expected.iter().cloned().collect(),
                        runtime.ready.iter().cloned().collect(),
                    )
                });
        expected.sort();
        ready.sort();
        describe(
            &self.snapshot(true),
            &self.root,
            Some(Liveness {
                pid,
                start_time: super::controller::process_identity(i64::from(pid)),
                expected,
                ready,
                correctable: self.correctable(),
            }),
        )
    }

    /// Route one request whose envelope `serve` checked; errors go to
    /// `reply`. An apply of a plan this host has neither prepared nor
    /// accepted is rejected here, before admission (exit 2).
    pub(super) fn request(
        &mut self,
        request: &Json,
        reply: &Reply,
        shutdown: bool,
    ) -> Result<(), Failure> {
        let checked = envelope(request).map_err(|(_, failure)| failure)?;
        let (operation, identity) = (checked.operation, checked.identity);
        if matches!(operation, "prepare" | "apply") && identity.is_empty() {
            return Err(fail(2, "request requires state identity"));
        }
        if !identity.is_empty() && identity != self.identity {
            return Err(fail(2, "request targets another host state"));
        }
        let payload = request.get("payload").cloned().unwrap_or(Json::Null);
        match operation {
            "snapshot" | "inspect" => {
                let mut status = if operation == "inspect" {
                    self.inspection()
                } else {
                    self.snapshot(true)
                };
                if let Some(id) = payload.as_str() {
                    if !self.receipts.contains_key(id) {
                        return Err(fail(2, format!("unknown operation {id}")));
                    }
                    if let Json::Object(fields) = &mut status {
                        fields.insert("operation".into(), self.receipt(id));
                    }
                }
                let _ = reply.send(Ok(status));
            }
            "prepare" if !shutdown => self.prepare(payload, reply.clone())?,
            "apply" if !shutdown => {
                let plan = Plan::parse(&payload).map_err(|reason| fail(2, reason))?;
                let recorded = self.root.join("plans").join(format!("{}.json", plan.id));
                if !self.receipts.contains_key(&plan.id)
                    && (plan.id.contains('/') || !recorded.is_file())
                {
                    return Err(fail(
                        2,
                        format!(
                            "plan {} was not prepared by this host; prepare it with `ignite plan --for-apply`",
                            plan.id
                        ),
                    ));
                }
                self.apply(payload, reply.clone())?
            }
            "recover" if !shutdown => self.operator_recover(&payload, reply.clone())?,
            _ => return Err(fail(2, "unsupported operation or host is stopping")),
        }
        Ok(())
    }
}
