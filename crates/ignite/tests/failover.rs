//! The headline of `docs/slice.md` section 14.0 item 3 on the LLVM target
//! (`docs/llvm.md` section 4): a service survives a session whose handler
//! fails an `assert`, overflows or indexes out of range, shown over a real
//! socket against `tests/fixtures/failover.ignite`; each failure is logged
//! once as `task ended with error: <report>`. And 10,000 failing sessions
//! leak neither handles nor arena memory, read from the server process's
//! `/proc` entry.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use ignite::Target;

use crate::common;

const TIMEOUT: Duration = Duration::from_secs(10);

fn source() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/failover.ignite")
}

/// `failoverd`, built once with the LLVM target.
fn failoverd() -> &'static PathBuf {
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    BUILT.get_or_init(|| {
        let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("llvm-failover");
        let options = ignite::BuildOptions {
            target: Target::Llvm,
            out: Some(out.clone()),
            runtime: Some(common::runtime_archive()),
            ..Default::default()
        };
        let output = ignite::build(&source(), &options).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(output.executables, [out.join("bin/failoverd")]);
        output.executables[0].clone()
    })
}

/// A running `failoverd`: its process, its port, and what it logged after
/// the `listening on` line.
struct Server {
    child: Child,
    port: u16,
    log: Arc<Mutex<String>>,
    reader: Option<JoinHandle<()>>,
}

impl Server {
    fn start() -> Server {
        let mut child = Command::new(failoverd())
            .args(["--port", "0"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn failoverd");
        let mut stderr = BufReader::new(child.stderr.take().unwrap());
        let mut first = String::new();
        stderr.read_line(&mut first).unwrap();
        let port = first
            .trim()
            .strip_prefix("listening on 127.0.0.1:")
            .unwrap_or_else(|| panic!("first line {first:?}"))
            .parse()
            .unwrap();
        let log = Arc::new(Mutex::new(String::new()));
        let sink = Arc::clone(&log);
        let reader = std::thread::spawn(move || {
            let mut rest = String::new();
            let _ = stderr.read_to_string(&mut rest);
            sink.lock().unwrap().push_str(&rest);
        });
        Server {
            child,
            port,
            log,
            reader: Some(reader),
        }
    }

    /// One session: send `line`, then everything the server writes until it
    /// closes the connection.
    fn session(&self, line: &str) -> Vec<u8> {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        stream.set_read_timeout(Some(TIMEOUT)).unwrap();
        stream.write_all(line.as_bytes()).unwrap();
        let mut reply = Vec::new();
        stream.read_to_end(&mut reply).unwrap();
        reply
    }

    fn alive(&mut self) -> bool {
        self.child.try_wait().expect("try_wait").is_none()
    }

    fn open_fds(&self) -> usize {
        std::fs::read_dir(format!("/proc/{}/fd", self.child.id()))
            .unwrap()
            .count()
    }

    fn rss_kib(&self) -> u64 {
        let status = std::fs::read_to_string(format!("/proc/{}/status", self.child.id())).unwrap();
        status
            .lines()
            .find_map(|line| line.strip_prefix("VmRSS:"))
            .expect("VmRSS")
            .trim()
            .trim_end_matches("kB")
            .trim()
            .parse()
            .unwrap()
    }

    /// End the server; its log after the `listening on` line.
    fn stop(mut self) -> String {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        self.log.lock().unwrap().clone()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn a_failing_session_does_not_end_the_service() {
    let mut server = Server::start();
    assert_eq!(server.session("hello\n"), b"ok hello\n");
    for command in ["index\n", "overflow\n", "assert\n"] {
        assert_eq!(server.session(command), b"", "{command:?} answered");
        assert!(server.alive(), "{command:?} ended the service");
        assert_eq!(server.session("after\n"), b"ok after\n", "{command:?}");
    }
    let log = server.stop();
    let file = source().display().to_string();
    for report in [
        format!("task ended with error: index failed at {file}:29:"),
        ": index 5 out of range for length 2\n".to_string(),
        format!("task ended with error: + failed at {file}:33:"),
        ": 9223372036854775807 + 9\n".to_string(),
        format!("task ended with error: assert failed at {file}:37:"),
        ": bytes.len() == 0\n  left: 7\n  right: 0\n".to_string(),
    ] {
        assert!(log.contains(&report), "no {report:?} in\n{log}");
    }
    assert_eq!(log.matches("task ended with error:").count(), 3, "{log}");
    assert!(!log.contains("error: error"), "{log}");
    assert!(!log.contains("session ended with error"), "{log}");
}

/// `docs/slice.md` section 14.8: after 10,000 failing sessions the server
/// holds the handles it held before and about the memory; a leaked 64 KiB
/// arena chunk per failure would be 640 MiB.
#[test]
fn ten_thousand_failing_sessions_leak_nothing() {
    let mut server = Server::start();
    assert_eq!(server.session("index\n"), b"");
    assert_eq!(server.session("warm\n"), b"ok warm\n");
    let fds = server.open_fds();
    let rss = server.rss_kib();
    for _ in 0..10_000 {
        assert_eq!(server.session("index\n"), b"");
    }
    assert!(server.alive());
    assert_eq!(server.session("still\n"), b"ok still\n");
    let deadline = Instant::now() + TIMEOUT;
    while server.open_fds() > fds && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    // The baseline may still count the warm session's socket, released a
    // moment after the client saw it close, so fewer is not a leak.
    let open = server.open_fds();
    assert!(open <= fds, "handles leaked: {open} open, {fds} before");
    let grown = server.rss_kib().saturating_sub(rss);
    assert!(
        grown < 32 * 1024,
        "RSS grew by {grown} KiB over 10,000 failing sessions"
    );
    let log = server.stop();
    assert_eq!(
        log.matches("task ended with error: index failed").count(),
        10_001,
        "{log}"
    );
}
