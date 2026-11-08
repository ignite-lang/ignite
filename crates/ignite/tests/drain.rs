//! The drain on SIGTERM (`docs/slice.md` 16.13) and `DrainRollout`
//! (16.0 item 6, 16.12), both targets.
//!
//! The drain is tested on `terminald` itself: an open session survives
//! SIGTERM, the listener's `released` line follows and a new connect is
//! refused, the process exits 0 once the session ends, and a second
//! SIGTERM ends it at once. The rollout is tested under `ignite run` with
//! the manual clock, through the shared `--build` of `run.rs` and a state
//! directory of its own: `rollout Terminal.server` builds, drains the old
//! process (SIGTERM, `released`), starts the new one, journals the
//! disruption, keeps the old session answering, and at `deadline` kills
//! what remains (`disconnect_remaining`, `retired forced`). No test
//! sleeps for a policy duration: the deadline is `advance`d (item 9).

use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::net::TcpStream;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use ignite_eval::Json;
use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;

use crate::common;

// The deadlines of this file. Each is a ceiling on a wait that returns at
// its event. Alone, `rollout_drains_the_old_process_and_starts_the_new_rust`
// takes 1.07 to 1.88 s whole and its `_llvm` twin 1.08 to 1.64 s, the cold
// builds included (5 runs each, 2026-11-07); both failed loaded gates of
// that day under the former 15 s.

/// The longest a test waits for one line, record, read or exit.
const TIMEOUT: Duration = Duration::from_secs(60);
/// The longest the rollout's `Build` waits for `Built`: the Rust target
/// builds with cargo in `run-cargo`, shared with `run::`, and queues on
/// its lock as the run's setup does ([`common::SETUP`]).
const BUILD: Duration = common::SETUP;
/// The longest a process killed and reaped takes to leave `/proc`.
const GONE: Duration = Duration::from_secs(2);
/// How long `assert_gone_fails_while_the_peer_holds_the_connection` reads
/// from a peer that holds its connection open before `assert_gone` fails.
const HELD: Duration = Duration::from_millis(200);
const DESIGN: &str = "examples/terminal/terminal.design.ignite";
/// The prompt as `std.protocol.telnet` sends it: the text, then IAC GA.
const PROMPT: &[u8] = b"terminal> \xff\xf9";
const TARGETS: [&str; 2] = ["rust", "llvm"];

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn tmp(name: impl AsRef<Path>) -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join(name)
}

/// One cargo target directory for every Rust-target build of this test
/// binary, shared with `run.rs`, so the substrate is compiled once.
fn cargo_target_dir() -> PathBuf {
    tmp("run-cargo")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn signal(pid: u32, signal: Signal) {
    kill(Pid::from_raw(pid as i32), signal).unwrap_or_else(|e| panic!("kill {signal} {pid}: {e}"));
}

/// `f` once per target, the two on their own threads; a failing target
/// fails the test with its own panic message.
fn on_both_targets(f: impl Fn(&str) + Sync) {
    std::thread::scope(|scope| {
        let handles: Vec<_> = TARGETS
            .iter()
            .map(|target| scope.spawn(|| f(target)))
            .collect();
        let failed = handles
            .into_iter()
            .map(|handle| handle.join())
            .filter(Result::is_err)
            .count();
        assert_eq!(failed, 0, "{failed} target(s) failed");
    });
}

/// Lines of a child's stream as they arrive.
fn lines_of(stream: impl Read + Send + 'static) -> Receiver<String> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stream).lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    rx
}

/// The next line satisfying `predicate`, within [`TIMEOUT`]; the lines
/// skipped are returned to the caller's `skipped`.
fn wait_line(
    lines: &Receiver<String>,
    what: &str,
    skipped: &mut Vec<String>,
    predicate: impl Fn(&str) -> bool,
) -> String {
    let started = Instant::now();
    loop {
        let left = TIMEOUT.saturating_sub(started.elapsed());
        match lines.recv_timeout(left) {
            Ok(line) if predicate(&line) => return line,
            Ok(line) => skipped.push(line),
            Err(RecvTimeoutError::Timeout) => {
                panic!(
                    "no {what} within {TIMEOUT:?}; lines seen:\n{}",
                    skipped.join("\n")
                )
            }
            Err(RecvTimeoutError::Disconnected) => {
                panic!(
                    "the stream ended before {what}; lines seen:\n{}",
                    skipped.join("\n")
                )
            }
        }
    }
}

/// Wait for a child's exit within [`TIMEOUT`].
fn wait_exit(child: &mut Child, what: &str) -> std::process::ExitStatus {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().expect("wait") {
            return status;
        }
        assert!(
            started.elapsed() < TIMEOUT,
            "{what} did not exit within {TIMEOUT:?}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// A telnet session with the terminal.
struct Session {
    stream: TcpStream,
    seen: Vec<u8>,
}

impl Session {
    /// Connect and read up to the prompt.
    fn open(port: u16) -> Session {
        let stream = TcpStream::connect(("127.0.0.1", port))
            .unwrap_or_else(|e| panic!("connect to {port}: {e}"));
        stream
            .set_read_timeout(Some(TIMEOUT))
            .expect("read timeout");
        let mut session = Session {
            stream,
            seen: Vec::new(),
        };
        session.read_until(|seen| seen.ends_with(PROMPT));
        session
    }

    fn read_until(&mut self, done: impl Fn(&[u8]) -> bool) {
        let mut buffer = [0u8; 256];
        while !done(&self.seen) {
            let n = self
                .stream
                .read(&mut buffer)
                .unwrap_or_else(|e| panic!("read: {e}; seen {:?}", text(&self.seen)));
            assert!(n > 0, "eof; seen {:?}", text(&self.seen));
            self.seen.extend_from_slice(&buffer[..n]);
        }
        self.seen.clear();
    }

    /// `echo <word>` answers `<word>` and a new prompt.
    fn echo(&mut self, word: &str) {
        self.stream
            .write_all(format!("echo {word}\r\n").as_bytes())
            .expect("write echo");
        let expected = word.to_string();
        self.read_until(|seen| text(seen).contains(&expected) && seen.ends_with(PROMPT));
    }

    fn quit(mut self) {
        let _ = self.stream.write_all(b"quit\r\n");
    }

    /// The session's peer is gone: the next read ends or the connection
    /// is reset; a read that times out with the connection open fails.
    fn assert_gone(&mut self) {
        self.assert_gone_within(TIMEOUT);
    }

    /// [`Session::assert_gone`] with a read timeout of `limit`.
    fn assert_gone_within(&mut self, limit: Duration) {
        self.stream
            .set_read_timeout(Some(limit))
            .expect("read timeout");
        let mut buffer = [0u8; 64];
        loop {
            match self.stream.read(&mut buffer) {
                Ok(0) => return,
                Err(e) if e.kind() == ErrorKind::ConnectionReset => return,
                Err(e) => panic!("the session is not gone ({e}); seen {:?}", text(&self.seen)),
                Ok(_) => continue,
            }
        }
    }
}

/// `ignite build` of the design file for `target` into this test's own
/// directory; the path of `terminald`.
fn build_terminald(target: &str) -> PathBuf {
    let out = tmp(format!("drain-build-{target}"));
    let mut command = Command::new(env!("CARGO_BIN_EXE_ignite"));
    command
        .arg("build")
        .arg(repo().join(DESIGN))
        .args(["--target", target, "--out"])
        .arg(&out)
        .current_dir(repo());
    if target == "llvm" {
        command.arg("--runtime").arg(common::runtime_archive());
    } else {
        command.arg("--target-dir").arg(cargo_target_dir());
    }
    let output = command.output().expect("run ignite build");
    let stdout = text(&output.stdout);
    assert!(
        output.status.success(),
        "{target}: ignite build failed\n{stdout}\n{}",
        text(&output.stderr)
    );
    stdout
        .lines()
        .filter_map(|line| line.strip_prefix("built "))
        .find(|path| path.ends_with("/terminald"))
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("{target}: no terminald in\n{stdout}"))
}

/// A `terminald` started on a free port, with its stderr lines.
struct Server {
    child: Child,
    stderr: Receiver<String>,
    skipped: Vec<String>,
    port: u16,
}

impl Server {
    fn start(terminald: &Path) -> Server {
        let mut child = Command::new(terminald)
            .args(["--port", "0"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|e| panic!("start {}: {e}", terminald.display()));
        let stderr = lines_of(child.stderr.take().expect("stderr"));
        let mut skipped = Vec::new();
        let listening = wait_line(&stderr, "listening on", &mut skipped, |l| {
            l.starts_with("listening on 127.0.0.1:")
        });
        let port = listening["listening on 127.0.0.1:".len()..]
            .parse()
            .unwrap_or_else(|e| panic!("{listening}: {e}"));
        Server {
            child,
            stderr,
            skipped,
            port,
        }
    }

    fn pid(&self) -> u32 {
        self.child.id()
    }

    fn wait_released(&mut self) {
        let expected = format!("released 127.0.0.1:{}", self.port);
        wait_line(&self.stderr, &expected, &mut self.skipped, |l| {
            l == expected
        });
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// 16.13: an open session survives SIGTERM; a new connect is refused after
/// `released`; the process exits 0 once the session ends; a second SIGTERM
/// ends the process at once.
#[test]
fn an_open_session_survives_sigterm_and_new_connects_are_refused() {
    on_both_targets(|target| {
        let terminald = build_terminald(target);

        let mut server = Server::start(&terminald);
        let mut session = Session::open(server.port);
        signal(server.pid(), Signal::SIGTERM);
        server.wait_released();
        let refused = TcpStream::connect(("127.0.0.1", server.port));
        assert!(
            refused.is_err(),
            "{target}: a connect after `released` succeeded"
        );
        session.echo("hello");
        session.echo("still here");
        let early = server.child.try_wait().expect("wait");
        assert!(
            early.is_none(),
            "{target}: the server exited with a session open: {early:?}"
        );
        session.quit();
        let status = wait_exit(&mut server.child, "the drained server");
        assert_eq!(status.code(), Some(0), "{target}: {status:?}");
        let released: Vec<&String> = server
            .skipped
            .iter()
            .filter(|l| l.starts_with("released "))
            .collect();
        assert!(
            released.is_empty(),
            "{target}: `released` printed twice: {released:?}"
        );

        let mut server = Server::start(&terminald);
        let _session = Session::open(server.port);
        signal(server.pid(), Signal::SIGTERM);
        server.wait_released();
        signal(server.pid(), Signal::SIGTERM);
        let status = wait_exit(&mut server.child, "the server after a second SIGTERM");
        assert_eq!(
            status.signal(),
            Some(Signal::SIGTERM as i32),
            "{target}: {status:?}"
        );
    });
}

/// The longest `Process.exit` may take to end a draining process (story
/// `drain-exit`).
const EXIT: Duration = Duration::from_secs(1);

/// `ignite build` of `tests/fixtures/<fixture>.ignite` for `target`; the
/// path of its executable `executable`.
fn build_fixture(fixture: &str, executable: &str, target: &str) -> PathBuf {
    let out = tmp(format!("{fixture}-build-{target}"));
    let mut command = Command::new(env!("CARGO_BIN_EXE_ignite"));
    command
        .arg("build")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/{fixture}.ignite")))
        .args(["--target", target, "--out"])
        .arg(&out)
        .current_dir(repo());
    if target == "llvm" {
        command.arg("--runtime").arg(common::runtime_archive());
    } else {
        command.arg("--target-dir").arg(cargo_target_dir());
    }
    let output = command.output().expect("run ignite build");
    let stdout = text(&output.stdout);
    assert!(
        output.status.success(),
        "{target}: ignite build failed\n{stdout}\n{}",
        text(&output.stderr)
    );
    stdout
        .lines()
        .filter_map(|line| line.strip_prefix("built "))
        .find(|path| path.ends_with(&format!("/{executable}")))
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("{target}: no {executable} in\n{stdout}"))
}

/// Story `drain-exit` (the case study's G16 and its G13 update): after
/// SIGTERM, `Process.exit(3)` ends the process with status 3 within
/// [`EXIT`] while one task waits in `accept` on a listener bound after the
/// drain began and another in an idle read, and the line written just
/// before it reaches the peer.
#[test]
fn process_exit_during_the_drain_exits_with_its_code_at_once() {
    on_both_targets(|target| {
        let drainexit = build_fixture("drain_exit", "drainexit", target);
        let mut server = Server::start(&drainexit);
        let _reading = TcpStream::connect(("127.0.0.1", server.port)).expect("connect reading");
        let mut peer = TcpStream::connect(("127.0.0.1", server.port)).expect("connect peer");
        peer.set_read_timeout(Some(TIMEOUT)).expect("read timeout");
        wait_line(&server.stderr, "waiting", &mut server.skipped, |l| {
            l == "waiting"
        });
        signal(server.pid(), Signal::SIGTERM);
        wait_line(&server.stderr, "exiting", &mut server.skipped, |l| {
            l == "exiting"
        });
        let exiting = Instant::now();
        let status = loop {
            if let Some(status) = server.child.try_wait().expect("wait") {
                break status;
            }
            assert!(
                exiting.elapsed() < EXIT,
                "{target}: still running {EXIT:?} after `exiting`; lines:\n{}",
                server.skipped.join("\n")
            );
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(status.code(), Some(3), "{target}: {status:?}");
        let mut received = Vec::new();
        peer.read_to_end(&mut received)
            .unwrap_or_else(|e| panic!("{target}: read the peer: {e}"));
        assert_eq!(text(&received), "bye\n", "{target}");
    });
}

/// Story `drain-exit` (the case study's G13 update): a line written just
/// before `main` returns during the drain reaches the peer while the
/// process still waits for a task in an idle read; the process exits 0
/// once that read ends.
#[test]
fn a_write_before_main_returns_during_the_drain_reaches_the_peer() {
    on_both_targets(|target| {
        let drainreturn = build_fixture("drain_return", "drainreturn", target);
        let mut server = Server::start(&drainreturn);
        let mut peer = TcpStream::connect(("127.0.0.1", server.port)).expect("connect peer");
        peer.set_read_timeout(Some(TIMEOUT)).expect("read timeout");
        wait_line(&server.stderr, "waiting", &mut server.skipped, |l| {
            l == "waiting"
        });
        signal(server.pid(), Signal::SIGTERM);
        wait_line(&server.stderr, "returning", &mut server.skipped, |l| {
            l == "returning"
        });
        let mut received = [0u8; 4];
        peer.read_exact(&mut received)
            .unwrap_or_else(|e| panic!("{target}: read the peer: {e}"));
        assert_eq!(text(&received), "bye\n", "{target}");
        let early = server.child.try_wait().expect("wait");
        assert!(
            early.is_none(),
            "{target}: exited with a task in a read: {early:?}"
        );
        peer.shutdown(std::net::Shutdown::Write)
            .expect("end the task's read");
        let status = wait_exit(&mut server.child, "the drained server");
        assert_eq!(status.code(), Some(0), "{target}: {status:?}");
    });
}

/// Adversary, story `drain-exit`: `Process.exit(263)` from a spawned task
/// during the drain, while `main` waits in an idle read, ends the process
/// with status 7 (263 modulo 256) on both targets within [`EXIT`], and the
/// child started after the drain began is killed (`at_exit`).
#[test]
fn adversary_process_exit_from_a_task_during_the_drain() {
    on_both_targets(|target| {
        let binary = build_fixture("drain_exit_task", "drainexittask", target);
        let mut server = Server::start(&binary);
        let _reading = TcpStream::connect(("127.0.0.1", server.port)).expect("connect reading");
        let mut trigger = TcpStream::connect(("127.0.0.1", server.port)).expect("connect trigger");
        wait_line(&server.stderr, "waiting", &mut server.skipped, |l| {
            l == "waiting"
        });
        signal(server.pid(), Signal::SIGTERM);
        let child = wait_line(&server.stderr, "child", &mut server.skipped, |l| {
            l.starts_with("child ")
        });
        let child: i32 = child["child ".len()..]
            .parse()
            .unwrap_or_else(|e| panic!("{target}: {child}: {e}"));
        trigger.write_all(b"go\n").expect("trigger");
        wait_line(&server.stderr, "task exiting", &mut server.skipped, |l| {
            l == "task exiting"
        });
        let exiting = Instant::now();
        let status = loop {
            if let Some(status) = server.child.try_wait().expect("wait") {
                break status;
            }
            assert!(
                exiting.elapsed() < EXIT,
                "{target}: still running {EXIT:?} after `task exiting`; lines:\n{}",
                server.skipped.join("\n")
            );
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(status.code(), Some(7), "{target}: {status:?}");
        let gone = Instant::now();
        while kill(Pid::from_raw(child), None).is_ok() {
            assert!(
                gone.elapsed() < GONE,
                "{target}: child {child} still alive {GONE:?} after the exit"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    });
}

/// Adversary, story `drain-exit`: `Process.exit(3)` during the drain ends
/// the process within 5 s (five times [`EXIT`], for load) although a task
/// that floods the same stream to a peer that never reads is stalled in
/// its send. `main`'s own line `bye` is pending on that stream, so the
/// exit's flush waits for the write lock the stalled task holds.
#[test]
fn adversary_process_exit_during_the_drain_with_a_stalled_writer() {
    on_both_targets(|target| {
        let binary = build_fixture("drain_exit_stall", "drainexitstall", target);
        let mut server = Server::start(&binary);
        let _slow = TcpStream::connect(("127.0.0.1", server.port)).expect("connect slow");
        wait_line(&server.stderr, "waiting", &mut server.skipped, |l| {
            l == "waiting"
        });
        signal(server.pid(), Signal::SIGTERM);
        wait_line(&server.stderr, "exiting", &mut server.skipped, |l| {
            l == "exiting"
        });
        let exiting = Instant::now();
        let limit = EXIT * 5;
        let status = loop {
            if let Some(status) = server.child.try_wait().expect("wait") {
                break status;
            }
            assert!(
                exiting.elapsed() < limit,
                "{target}: still running {limit:?} after `exiting`; lines:\n{}",
                server.skipped.join("\n")
            );
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(status.code(), Some(3), "{target}: {status:?}");
    });
}

/// The next `listening on 127.0.0.1:<port>` line of `server`; its port.
fn next_listening_port(server: &mut Server) -> u16 {
    let listening = wait_line(&server.stderr, "listening on", &mut server.skipped, |l| {
        l.starts_with("listening on 127.0.0.1:")
    });
    listening["listening on 127.0.0.1:".len()..]
        .parse()
        .unwrap_or_else(|e| panic!("{listening}: {e}"))
}

/// Connect to `port` and read the kept listener's greeting `internal`.
fn greeted_by_internal(target: &str, port: u16, when: &str) -> TcpStream {
    let mut stream = TcpStream::connect(("127.0.0.1", port))
        .unwrap_or_else(|e| panic!("{target}: connect to internal {when}: {e}"));
    stream
        .set_read_timeout(Some(TIMEOUT))
        .expect("read timeout");
    let mut greeting = [0u8; 9];
    stream
        .read_exact(&mut greeting)
        .unwrap_or_else(|e| panic!("{target}: read internal's greeting {when}: {e}"));
    assert_eq!(text(&greeting), "internal\n", "{target}: {when}");
    stream
}

/// Story `drain-keeps-listeners` (the case study's G6): after SIGTERM the
/// listener declared `keep_through_drain = true` still accepts and answers
/// and is never released, while the ingress listener is released and its
/// pending `accept` fails with "draining". The process still exits 0 once
/// the one ingress connection is done, although a task waits in the kept
/// listener's `accept` and another in an idle read of a kept connection.
#[test]
fn a_kept_listener_accepts_through_the_drain_and_the_others_are_released() {
    on_both_targets(|target| {
        let binary = build_fixture("drain_keep", "drainkeep", target);
        let mut server = Server::start(&binary);
        let ingress = server.port;
        let internal = next_listening_port(&mut server);
        let held = TcpStream::connect(("127.0.0.1", ingress)).expect("connect ingress");
        wait_line(&server.stderr, "waiting", &mut server.skipped, |l| {
            l == "waiting"
        });
        let _idle_kept = greeted_by_internal(target, internal, "before the drain");
        signal(server.pid(), Signal::SIGTERM);
        server.wait_released();
        let draining = "ingress accept: draining";
        if !server.skipped.iter().any(|l| l == draining) {
            wait_line(&server.stderr, draining, &mut server.skipped, |l| {
                l == draining
            });
        }
        let refused = TcpStream::connect(("127.0.0.1", ingress));
        assert!(
            refused.is_err(),
            "{target}: a connect to ingress after `released` succeeded"
        );
        for round in 0..3 {
            greeted_by_internal(target, internal, &format!("in the drain, round {round}"));
        }
        let early = server.child.try_wait().expect("wait");
        assert!(
            early.is_none(),
            "{target}: exited with the ingress connection open: {early:?}"
        );
        held.shutdown(std::net::Shutdown::Write)
            .expect("end the ingress connection");
        let status = wait_exit(&mut server.child, "the drained server");
        assert_eq!(status.code(), Some(0), "{target}: {status:?}");
        server.skipped.extend(server.stderr.iter());
        let kept = format!("released 127.0.0.1:{internal}");
        let released: Vec<&String> = server
            .skipped
            .iter()
            .filter(|l| l.starts_with("released "))
            .collect();
        assert!(
            !released.iter().any(|l| **l == kept),
            "{target}: the kept listener was released: {released:?}"
        );
    });
}

/// The port of the `<name> on <port>` line of `stderr`.
fn bound_port(stderr: &Receiver<String>, name: &str, skipped: &mut Vec<String>) -> u16 {
    let prefix = format!("{name} on ");
    let line = wait_line(stderr, &prefix, skipped, |l| l.starts_with(&prefix));
    line[prefix.len()..]
        .parse()
        .unwrap_or_else(|e| panic!("{line}: {e}"))
}

/// Story `drain-keeps-bound-listeners` (the case study's G31): of two
/// listeners bound at run time on ports read from the environment, the one
/// bound with `TcpListener.bind` is released at SIGTERM and the one bound
/// with `TcpListener.bind_kept` prints no `released` line and still
/// accepts and answers. The process exits 0 once the one public connection
/// is done, although a task waits in the kept listener's `accept` and
/// another in an idle read of a kept connection.
#[test]
fn a_listener_bound_kept_at_run_time_accepts_through_the_drain() {
    on_both_targets(|target| {
        let binary = build_fixture("drain_bind_kept", "drainbindkept", target);
        let mut child = Command::new(&binary)
            .env("PUBLIC_PORT", "0")
            .env("INTERNAL_PORT", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|e| panic!("start {}: {e}", binary.display()));
        let stderr = lines_of(child.stderr.take().expect("stderr"));
        let mut server = Server {
            child,
            stderr,
            skipped: Vec::new(),
            port: 0,
        };
        server.port = bound_port(&server.stderr, "public", &mut server.skipped);
        let public = server.port;
        let internal = bound_port(&server.stderr, "internal", &mut server.skipped);
        let held = TcpStream::connect(("127.0.0.1", public)).expect("connect public");
        wait_line(&server.stderr, "waiting", &mut server.skipped, |l| {
            l == "waiting"
        });
        let _idle_kept = greeted_by_internal(target, internal, "before the drain");
        signal(server.pid(), Signal::SIGTERM);
        server.wait_released();
        let draining = "public accept: draining";
        if !server.skipped.iter().any(|l| l == draining) {
            wait_line(&server.stderr, draining, &mut server.skipped, |l| {
                l == draining
            });
        }
        let refused = TcpStream::connect(("127.0.0.1", public));
        assert!(
            refused.is_err(),
            "{target}: a connect to public after `released` succeeded"
        );
        for round in 0..3 {
            greeted_by_internal(target, internal, &format!("in the drain, round {round}"));
        }
        let early = server.child.try_wait().expect("wait");
        assert!(
            early.is_none(),
            "{target}: exited with the public connection open: {early:?}"
        );
        held.shutdown(std::net::Shutdown::Write)
            .expect("end the public connection");
        let status = wait_exit(&mut server.child, "the drained server");
        assert_eq!(status.code(), Some(0), "{target}: {status:?}");
        server.skipped.extend(server.stderr.iter());
        let kept = format!("released 127.0.0.1:{internal}");
        let released: Vec<&String> = server
            .skipped
            .iter()
            .filter(|l| l.starts_with("released "))
            .collect();
        assert!(
            !released.iter().any(|l| **l == kept),
            "{target}: the kept listener was released: {released:?}"
        );
    });
}

/// Every `.ll` file under `dir`, recursively.
fn ll_files(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            ll_files(&path, found);
        } else if path.extension().is_some_and(|e| e == "ll") {
            found.push(path);
        }
    }
}

/// Adversary of story `drain-keeps-bound-listeners`: several listeners at
/// once. Two bound with `TcpListener.bind` are both `released`; two bound
/// with `bind_kept`, the `keep_through_drain` resource `admin` and one
/// bound with `bind_kept` after the drain began all answer through the
/// drain, none of them is `released`, and the process exits 0. On LLVM,
/// every IR module declares `ignite_rt_tcp_bind_kept` at most once.
#[test]
fn adversary_several_kept_and_released_listeners_at_once() {
    on_both_targets(|target| {
        let binary = build_fixture("drain_bind_kept_many", "drainbindkeptmany", target);
        if target == "llvm" {
            let mut files = Vec::new();
            ll_files(
                &tmp(format!("drain_bind_kept_many-build-{target}")),
                &mut files,
            );
            assert!(!files.is_empty(), "{target}: no .ll files");
            for file in files {
                let ir = std::fs::read_to_string(&file).expect("read ir");
                let declared = ir
                    .lines()
                    .map(|l| l.replace('"', ""))
                    .filter(|l| l.starts_with("declare") && l.contains("@ignite_rt_tcp_bind_kept("))
                    .count();
                assert!(declared <= 1, "{}: {declared} declarations", file.display());
            }
        }
        let mut child = Command::new(&binary)
            .env("PLAIN_A_PORT", "0")
            .env("KEPT_A_PORT", "0")
            .env("PLAIN_B_PORT", "0")
            .env("KEPT_B_PORT", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|e| panic!("start {}: {e}", binary.display()));
        let stderr = lines_of(child.stderr.take().expect("stderr"));
        let mut server = Server {
            child,
            stderr,
            skipped: Vec::new(),
            port: 0,
        };
        let admin = next_listening_port(&mut server);
        let plain_a = bound_port(&server.stderr, "plain_a", &mut server.skipped);
        let kept_a = bound_port(&server.stderr, "kept_a", &mut server.skipped);
        let plain_b = bound_port(&server.stderr, "plain_b", &mut server.skipped);
        let kept_b = bound_port(&server.stderr, "kept_b", &mut server.skipped);
        let held = TcpStream::connect(("127.0.0.1", plain_a)).expect("connect plain_a");
        wait_line(&server.stderr, "waiting", &mut server.skipped, |l| {
            l == "waiting"
        });
        let _idle = greeted_by_internal(target, kept_a, "kept_a before the drain");
        signal(server.pid(), Signal::SIGTERM);
        for port in [plain_a, plain_b] {
            server.port = port;
            let expected = format!("released 127.0.0.1:{port}");
            if !server.skipped.contains(&expected) {
                server.wait_released();
            }
        }
        let late = match server.skipped.iter().find(|l| l.starts_with("late on ")) {
            Some(line) => line["late on ".len()..]
                .parse()
                .unwrap_or_else(|e| panic!("{line}: {e}")),
            None => bound_port(&server.stderr, "late", &mut server.skipped),
        };
        for (name, port) in [("plain_a", plain_a), ("plain_b", plain_b)] {
            assert!(
                TcpStream::connect(("127.0.0.1", port)).is_err(),
                "{target}: a connect to {name} after `released` succeeded"
            );
        }
        for round in 0..2 {
            for (name, port) in [
                ("kept_a", kept_a),
                ("kept_b", kept_b),
                ("admin", admin),
                ("late", late),
            ] {
                greeted_by_internal(target, port, &format!("{name} in the drain, round {round}"));
            }
        }
        let early = server.child.try_wait().expect("wait");
        assert!(
            early.is_none(),
            "{target}: exited with the plain_a connection open: {early:?}"
        );
        held.shutdown(std::net::Shutdown::Write)
            .expect("end the plain_a connection");
        let status = wait_exit(&mut server.child, "the drained server");
        assert_eq!(status.code(), Some(0), "{target}: {status:?}");
        server.skipped.extend(server.stderr.iter());
        let released: Vec<&String> = server
            .skipped
            .iter()
            .filter(|l| l.starts_with("released "))
            .collect();
        for port in [kept_a, kept_b, admin, late] {
            let kept = format!("released 127.0.0.1:{port}");
            assert!(
                !released.iter().any(|l| **l == kept),
                "{target}: kept listener {port} was released: {released:?}"
            );
        }
    });
}

/// Runs of a `flood_*` fixture per target.
const FLOOD_RUNS: usize = 30;

/// Run `binary` [`FLOOD_RUNS`] times with a peer that reads everything at
/// full speed; the runs whose peer never received `MARK\n`, and the exit
/// statuses seen.
fn flood_runs_losing_mark(binary: &Path) -> (usize, Vec<Option<i32>>) {
    let mut lost = 0;
    let mut codes = Vec::new();
    for _ in 0..FLOOD_RUNS {
        let mut server = Server::start(binary);
        let mut peer = TcpStream::connect(("127.0.0.1", server.port)).expect("connect peer");
        peer.set_read_timeout(Some(TIMEOUT)).expect("read timeout");
        let reader = std::thread::spawn(move || {
            let mut received = Vec::new();
            let _ = peer.read_to_end(&mut received);
            received
        });
        let status = wait_exit(&mut server.child, "flood");
        codes.push(status.code());
        let received = reader.join().expect("reader");
        if !received.windows(5).any(|w| w == b"MARK\n") {
            lost += 1;
        }
    }
    (lost, codes)
}

/// Adversary, story `drain-exit`, pass 2: `Process.exit(3)` after `main`
/// wrote `MARK` on a stream that another task writes to in a loop, with a
/// peer that reads everything. The other task holds the write lock for
/// one append at a time, not stalled; before round 2 exit's flush waited
/// for the lock and sent `MARK`. `flush_at_exit` skips the stream when the
/// lock is held at that instant.
#[test]
fn adversary_process_exit_sends_main_s_write_beside_a_busy_writer() {
    on_both_targets(|target| {
        let binary = build_fixture("flood_exit", "floodexit", target);
        let (lost, codes) = flood_runs_losing_mark(&binary);
        assert!(
            codes.iter().all(|code| *code == Some(3)),
            "{target}: {codes:?}"
        );
        assert_eq!(
            lost, 0,
            "{target}: MARK lost in {lost} of {FLOOD_RUNS} runs with a reading peer"
        );
    });
}

/// Adversary, story `drain-exit`, pass 2: the same, but `main` returns
/// `Ok` (no drain). The Rust target's `run` waits for the lock
/// (`ignite_io::flush`); the LLVM target's `ignite_rt_exit` ends in
/// `ignite_io::exit`, now `flush_at_exit`, which skips a held lock. The
/// targets must agree.
#[test]
fn adversary_main_return_sends_main_s_write_beside_a_busy_writer() {
    on_both_targets(|target| {
        let binary = build_fixture("flood_return", "floodreturn", target);
        let (lost, codes) = flood_runs_losing_mark(&binary);
        assert!(
            codes.iter().all(|code| *code == Some(0)),
            "{target}: {codes:?}"
        );
        assert_eq!(
            lost, 0,
            "{target}: MARK lost in {lost} of {FLOOD_RUNS} runs with a reading peer"
        );
    });
}

// ---- 16.0 item 6: DrainRollout under `ignite run` -------------------------

/// A running `ignite run` with its journal records as they arrive.
struct Run {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
    pending_initial: Option<String>,
    stderr: Receiver<String>,
    seen: Vec<Json>,
    state: PathBuf,
}

impl Run {
    /// `ignite run <design> --assembly local --state <fresh> --build
    /// <shared with run.rs> --target <target> --json --settle 1 s --clock
    /// manual --port Terminal.ingress=0`.
    fn design(target: &str, name: &str) -> Run {
        let state = tmp(format!("run-state-{name}"));
        let _ = std::fs::remove_dir_all(&state);
        let mut command = Command::new(env!("CARGO_BIN_EXE_ignite"));
        command
            .arg("run")
            .arg(repo().join(DESIGN))
            .args(["--assembly", "local", "--state"])
            .arg(&state)
            .arg("--build")
            .arg(tmp(format!("run-build-design-{target}")))
            .args(["--target", target, "--json", "--settle", "1 s"])
            .args(["--clock", "manual", "--port", "Terminal.ingress=0"]);
        if target == "llvm" {
            command.arg("--runtime").arg(common::runtime_archive());
        } else {
            command.arg("--target-dir").arg(cargo_target_dir());
        }
        let mut child = command
            .current_dir(repo())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn ignite run");
        let stdin = child.stdin.take().expect("stdin");
        let lines = lines_of(child.stdout.take().expect("stdout"));
        let stderr = lines_of(child.stderr.take().expect("stderr"));
        let mut run = Run {
            child,
            stdin,
            lines,
            pending_initial: None,
            stderr,
            seen: Vec::new(),
            state,
        };
        run.pending_initial = Some(common::wait_run_initial_segment(&run.lines, || {
            run.stderr()
        }));
        run
    }

    fn stderr(&self) -> String {
        let mut out = Vec::new();
        while let Ok(line) = self.stderr.try_recv() {
            out.push(line);
        }
        out.join("\n")
    }

    fn send(&mut self, line: &str) {
        writeln!(self.stdin, "{line}").expect("write control line");
        self.stdin.flush().expect("flush control line");
    }

    /// Consume records until one satisfies `predicate`, within [`TIMEOUT`];
    /// that record.
    fn wait_for(&mut self, what: &str, predicate: impl Fn(&Json) -> bool) -> Json {
        self.wait_within(what, TIMEOUT, predicate)
    }

    /// Consume records until one satisfies `predicate`, within `limit`;
    /// that record.
    fn wait_within(
        &mut self,
        what: &str,
        limit: Duration,
        predicate: impl Fn(&Json) -> bool,
    ) -> Json {
        let started = Instant::now();
        loop {
            let left = limit.saturating_sub(started.elapsed());
            let line = match self
                .pending_initial
                .take()
                .map(Ok)
                .unwrap_or_else(|| self.lines.recv_timeout(left))
            {
                Ok(line) => line,
                Err(RecvTimeoutError::Timeout) => panic!(
                    "no {what} within {limit:?}; seen {} records, the last {:?}\nstderr:\n{}",
                    self.seen.len(),
                    self.seen.last().map(Json::compact),
                    self.stderr()
                ),
                Err(RecvTimeoutError::Disconnected) => panic!(
                    "ignite run ended before {what}; seen {} records\nstderr:\n{}",
                    self.seen.len(),
                    self.stderr()
                ),
            };
            let record = Json::parse(&line).unwrap_or_else(|e| panic!("{line}: {e}"));
            self.seen.push(record.clone());
            if predicate(&record) {
                return record;
            }
        }
    }

    fn wait_event(&mut self, machine_id: &str, variant: &str) -> Json {
        self.wait_for(&format!("event {variant} of {machine_id}"), |r| {
            machine(r) == machine_id && event_variant(r) == Some(variant)
        })
    }

    fn wait_effect(&mut self, machine_id: &str, variant: &str) -> Json {
        self.wait_for(&format!("effect {variant} of {machine_id}"), |r| {
            machine(r) == machine_id && kind(r) == "effect" && effect_variant(r) == Some(variant)
        })
    }

    fn wait_note(&mut self, machine_id: &str, contains: &str) -> Json {
        self.wait_for(&format!("note `{contains}` of {machine_id}"), |r| {
            machine(r) == machine_id && note_text(r).is_some_and(|t| t.contains(contains))
        })
    }

    /// `status`; the port of `Terminal.ingress` it reports.
    fn port(&mut self) -> u16 {
        self.send("status");
        let status = self.wait_for("status", |r| kind(r) == "status");
        let port = status
            .get("ports")
            .and_then(|p| p.get("Terminal.ingress"))
            .and_then(Json::as_i64)
            .unwrap_or_else(|| panic!("no port in {}", status.compact()));
        assert!(port > 0 && port != 2323, "port {port}");
        port as u16
    }

    /// `stop`, then the exit status.
    fn stop(&mut self) -> i32 {
        self.send("stop");
        let status = wait_exit(&mut self.child, "ignite run");
        status.code().unwrap_or(-1)
    }

    /// Item 7: the journal replays with exit 0.
    fn replay(&self) {
        let output = Command::new(env!("CARGO_BIN_EXE_ignite"))
            .arg("replay")
            .arg(repo().join(DESIGN))
            .args(["--assembly", "local", "--state"])
            .arg(&self.state)
            .current_dir(repo())
            .output()
            .expect("run ignite replay");
        assert_eq!(
            output.status.code(),
            Some(0),
            "replay of {}:\n{}",
            self.state.display(),
            text(&output.stderr)
        );
    }
}

impl Drop for Run {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn kind(record: &Json) -> &str {
    record.get("kind").and_then(Json::as_str).unwrap_or("")
}

fn machine(record: &Json) -> &str {
    record.get("machine").and_then(Json::as_str).unwrap_or("")
}

fn event_kind(record: &Json) -> Option<&Json> {
    if kind(record) != "event" {
        return None;
    }
    record.get("value")?.get("fields")?.get("kind")
}

fn event_variant(record: &Json) -> Option<&str> {
    event_kind(record)?.get("variant")?.as_str()
}

fn event_fields(record: &Json) -> &[Json] {
    event_kind(record)
        .and_then(|k| k.get("fields"))
        .and_then(Json::as_array)
        .unwrap_or(&[])
}

fn effect_variant(record: &Json) -> Option<&str> {
    record.get("value")?.get("variant")?.as_str()
}

fn effect_fields(record: &Json) -> &[Json] {
    record
        .get("value")
        .and_then(|v| v.get("fields"))
        .and_then(Json::as_array)
        .unwrap_or(&[])
}

fn note_text(record: &Json) -> Option<&str> {
    if kind(record) != "effect" || effect_variant(record) != Some("Note") {
        return None;
    }
    effect_fields(record).first()?.as_str()
}

fn at_ns(record: &Json) -> i64 {
    record
        .get("at")
        .and_then(|at| at.get("ns"))
        .and_then(Json::as_i64)
        .unwrap_or_else(|| panic!("no at in {}", record.compact()))
}

/// The process of `pid` is gone or a zombie, within [`GONE`].
fn assert_no_process(pid: i64) {
    let proc_dir = PathBuf::from(format!("/proc/{pid}"));
    let started = Instant::now();
    loop {
        let status = std::fs::read_to_string(proc_dir.join("status")).unwrap_or_default();
        let zombie = status
            .lines()
            .any(|l| l.starts_with("State:") && l.contains('Z'));
        if !proc_dir.exists() || zombie {
            return;
        }
        assert!(
            started.elapsed() < GONE,
            "pid {pid} is still alive:\n{status}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// 16.0 item 6 on one target: `rollout Terminal.server` builds, drains the
/// old process with SIGTERM (its `released` line reaches the run as
/// `Released`), spawns the new generation, journals the disruption from
/// `Released` to the new `Ready`, keeps the old session answering, and at
/// the deadline (`advance 2 minutes`, manual clock) kills what remains:
/// `retired forced`, the old pid gone, the old session cut. Then `stop`
/// exits 0 and the journal replays (item 7).
fn rollout_drains_and_restarts(target: &str) {
    const SUPERVISOR: &str = "Terminal.server.supervisor";
    const ROLLOUT: &str = "Terminal.server.rollout";
    let mut run = Run::design(target, &format!("item6-{target}"));
    let started = run.wait_event(SUPERVISOR, "Started");
    let old_pid = event_fields(&started)[0]
        .as_i64()
        .unwrap_or_else(|| panic!("{}", started.compact()));
    run.wait_event(SUPERVISOR, "Ready");
    let port = run.port();
    let mut old_session = Session::open(port);
    old_session.echo("before");

    run.send("rollout Terminal.server");
    let build = run.wait_effect(ROLLOUT, "Build");
    assert_eq!(effect_fields(&build)[0].as_str(), Some("Terminal.server"));
    let built = run.wait_within("event Built of the rollout", BUILD, |r| {
        machine(r) == ROLLOUT && event_variant(r) == Some("Built")
    });
    let generation = event_fields(&built)[0].as_i64().unwrap_or(0);
    assert!(generation >= 2, "{}", built.compact());
    let term = run.wait_effect(ROLLOUT, "Kill");
    assert_eq!(
        (
            effect_fields(&term)[1].as_i64(),
            effect_fields(&term)[2].as_i64()
        ),
        (Some(old_pid), Some(Signal::SIGTERM as i64)),
        "{}",
        term.compact()
    );
    run.wait_effect(ROLLOUT, "AwaitRelease");
    let released = run.wait_event(ROLLOUT, "Released");
    let released_at = at_ns(&released);
    let spawn = run.wait_effect(ROLLOUT, "Spawn");
    assert_eq!(
        effect_fields(&spawn)[1].as_i64(),
        Some(generation),
        "{}",
        spawn.compact()
    );
    let new_started = run.wait_event(SUPERVISOR, "Started");
    let new_pid = event_fields(&new_started)[0].as_i64().unwrap_or(0);
    assert_ne!(new_pid, old_pid);
    assert_eq!(event_fields(&new_started)[1].as_i64(), Some(generation));
    let new_ready = run.wait_event(SUPERVISOR, "Ready");
    let disruption = run.wait_note(ROLLOUT, "disruption");
    assert_eq!(
        at_ns(&disruption),
        at_ns(&new_ready),
        "the disruption is journaled at the new Ready: {}",
        disruption.compact()
    );
    assert!(
        at_ns(&new_ready) >= released_at,
        "released at {released_at}, ready at {}",
        at_ns(&new_ready)
    );

    old_session.echo("during");
    let new_port = run.port();
    let mut new_session = Session::open(new_port);
    new_session.echo("new");
    new_session.quit();

    run.send("advance 2 minutes");
    let forced = run.wait_effect(ROLLOUT, "Kill");
    assert_eq!(
        (
            effect_fields(&forced)[1].as_i64(),
            effect_fields(&forced)[2].as_i64()
        ),
        (Some(old_pid), Some(Signal::SIGKILL as i64)),
        "{}",
        forced.compact()
    );
    run.wait_note(ROLLOUT, "retired forced");
    let signalled = run.wait_event(SUPERVISOR, "Signalled");
    assert_eq!(
        event_fields(&signalled)[0].as_i64(),
        Some(old_pid),
        "{}",
        signalled.compact()
    );
    assert_no_process(old_pid);
    old_session.assert_gone();

    let code = run.stop();
    assert_eq!(code, 0, "stderr:\n{}", run.stderr());
    assert_no_process(new_pid);
    run.replay();
}

#[test]
fn rollout_drains_the_old_process_and_starts_the_new_rust() {
    rollout_drains_and_restarts("rust");
}

#[test]
fn rollout_drains_the_old_process_and_starts_the_new_llvm() {
    rollout_drains_and_restarts("llvm");
}

/// Adversary (story `apply-test-deadlines`): `Session::assert_gone` must
/// fail when the peer keeps the connection open and silent, the state a
/// session descriptor leaked into the new generation leaves behind. Its
/// read timeout returns `Err`, which the helper once took for "gone"; the
/// read times out after [`HELD`], not [`TIMEOUT`].
#[test]
#[should_panic(expected = "the session is not gone")]
fn assert_gone_fails_while_the_peer_holds_the_connection() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("port").port();
    let stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    let (_peer, _) = listener.accept().expect("accept");
    let mut session = Session {
        stream,
        seen: Vec::new(),
    };
    session.assert_gone_within(HELD);
}

/// Adversary pass 2 (story `apply-test-deadlines`): the legitimate close
/// paths of a SIGKILLed peer still count as gone. A peer that closes with
/// nothing unread sends FIN (`Ok(0)`).
#[test]
fn assert_gone_returns_when_the_peer_closes() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("port").port();
    let stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    let (peer, _) = listener.accept().expect("accept");
    let mut session = Session {
        stream,
        seen: Vec::new(),
    };
    let started = std::time::Instant::now();
    drop(peer);
    session.assert_gone();
    assert!(started.elapsed() < Duration::from_secs(5));
}

/// Adversary pass 2: a peer that closes with bytes unread in its receive
/// buffer (the old session's input not yet read when SIGKILL arrives)
/// sends RST, after data the client has not read yet.
#[test]
fn assert_gone_returns_when_the_peer_resets_after_data() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("port").port();
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    let (mut peer, _) = listener.accept().expect("accept");
    peer.write_all(b"terminal> ").expect("peer write");
    stream.write_all(b"echo unread\r\n").expect("client write");
    std::thread::sleep(Duration::from_millis(100));
    let mut session = Session {
        stream,
        seen: Vec::new(),
    };
    let started = std::time::Instant::now();
    drop(peer);
    session.assert_gone();
    assert!(started.elapsed() < Duration::from_secs(5));
}
