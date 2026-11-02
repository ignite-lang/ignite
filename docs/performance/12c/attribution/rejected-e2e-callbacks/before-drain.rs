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

use std::io::{BufRead, BufReader, Read, Write};
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

/// The longest a test waits for one line, record or exit.
const TIMEOUT: Duration = Duration::from_secs(15);
const DESIGN: &str = "examples/terminal/terminal.design.ignite";
const PROMPT: &[u8] = b"terminal> ";
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
            .set_read_timeout(Some(Duration::from_secs(5)))
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

    /// The session's peer is gone: the next read ends or fails.
    fn assert_gone(&mut self) {
        self.stream
            .set_read_timeout(Some(TIMEOUT))
            .expect("read timeout");
        let mut buffer = [0u8; 64];
        loop {
            match self.stream.read(&mut buffer) {
                Ok(0) | Err(_) => return,
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

// ---- 16.0 item 6: DrainRollout under `ignite run` -------------------------

/// A running `ignite run` with its journal records as they arrive.
struct Run {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
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
        Run {
            child,
            stdin,
            lines,
            stderr,
            seen: Vec::new(),
            state,
        }
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

    /// Consume records until one satisfies `predicate`; that record.
    fn wait_for(&mut self, what: &str, predicate: impl Fn(&Json) -> bool) -> Json {
        let started = Instant::now();
        loop {
            let left = TIMEOUT.saturating_sub(started.elapsed());
            let line = match self.lines.recv_timeout(left) {
                Ok(line) => line,
                Err(RecvTimeoutError::Timeout) => panic!(
                    "no {what} within {TIMEOUT:?}; seen {} records, the last {:?}\nstderr:\n{}",
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

/// The process of `pid` is gone or a zombie, within 2 s.
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
            started.elapsed() < Duration::from_secs(2),
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
    let built = run.wait_event(ROLLOUT, "Built");
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
