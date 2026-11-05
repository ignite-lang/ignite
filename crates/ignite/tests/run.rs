//! `ignite run` and `ignite replay` end to end (`docs/slice.md` 16.0 items
//! 1 to 5 and 7 to 9; 16.17 E125 to E128). Every run uses the manual
//! clock unless the test is about the real one, passes `--settle 1 s`
//! (O29, A2), binds `Terminal.ingress` to a free port (never 2323) and
//! synchronizes on the journal records `--json` prints. The design file is
//! built once per target into one shared `--build`; each run has its own
//! `--state`; every journal a test writes is replayed (item 7). The `at` of
//! `Ready`, `Started` and `AttemptDone` is asserted only as the instant of
//! the dispatch (A2). Only [`probe_timeout_is_visible_in_journal_and_standing`]
//! pays a settle wait (16.4).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Output, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ignite_eval::Json;

use crate::common;

/// The longest a test waits for one record or an exit.
const TIMEOUT: Duration = Duration::from_secs(15);
const DESIGN: &str = "examples/terminal/terminal.design.ignite";
const PROMPT: &[u8] = b"terminal> ";
const NS_PER_MS: i64 = 1_000_000;
const SIGKILL: i32 = 9;
const SIGSTOP: i32 = 19;
const SIGCONT: i32 = 18;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn fixture(name: &str) -> PathBuf {
    fixtures_dir().join(name)
}

/// A fresh state directory for one run.
fn state(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("run-state-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// The shared build directory of `program` for `target` (16.3: tests build
/// once per target and never rebuild).
fn build_dir(program: &str, target: &str) -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("run-build-{program}-{target}"))
}

/// One cargo target directory for every Rust-target run of this test
/// binary, so the substrate is compiled once.
fn cargo_target_dir() -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join("run-cargo")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn send_signal(pid: i64, signal: i32) {
    let status = Command::new("kill")
        .arg(format!("-{signal}"))
        .arg(pid.to_string())
        .status()
        .expect("run kill");
    assert!(status.success(), "kill -{signal} {pid} failed");
}

/// A running `ignite run` with its journal records as they arrive.
struct Run {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Receiver<String>,
    pending_initial: Option<String>,
    stderr: Arc<Mutex<String>>,
    /// Every record consumed so far, in order.
    seen: Vec<Json>,
    state: PathBuf,
    file: PathBuf,
    extra: Vec<String>,
}

impl Run {
    /// Start `ignite run file --assembly local --state <fresh> --build
    /// <shared> --json --settle 1 s` plus `args`; the design file also gets
    /// `--port Terminal.ingress=0`.
    fn start(file: &Path, program: &str, target: &str, name: &str, args: &[&str]) -> Run {
        Run::start_in(&repo(), file, program, target, name, args)
    }

    /// [`Run::start`] in the working directory `dir`.
    fn start_in(
        dir: &Path,
        file: &Path,
        program: &str,
        target: &str,
        name: &str,
        args: &[&str],
    ) -> Run {
        let state = state(name);
        let mut command = Command::new(env!("CARGO_BIN_EXE_ignite"));
        command
            .arg("run")
            .arg(file)
            .args(["--assembly", "local", "--state"])
            .arg(&state)
            .arg("--build")
            .arg(build_dir(program, target))
            .args(["--target", target, "--json", "--settle", "1 s"]);
        if target == "llvm" {
            command.arg("--runtime").arg(common::runtime_archive());
        } else {
            command.arg("--target-dir").arg(cargo_target_dir());
        }
        if program == "design" {
            command.args(["--port", "Terminal.ingress=0"]);
        }
        command.args(args);
        command
            .current_dir(dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().expect("spawn ignite run");
        let stdin = child.stdin.take();
        let stdout = child.stdout.take().expect("stdout");
        let (tx, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let stderr = Arc::new(Mutex::new(String::new()));
        let sink = Arc::clone(&stderr);
        let mut err = child.stderr.take().expect("stderr");
        std::thread::spawn(move || {
            let mut buffer = [0u8; 4096];
            loop {
                match err.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => sink
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .push_str(&text(&buffer[..n])),
                }
            }
        });
        let mut run = Run {
            child,
            stdin,
            lines,
            pending_initial: None,
            stderr,
            seen: Vec::new(),
            state,
            file: file.to_path_buf(),
            extra: args.iter().map(|a| a.to_string()).collect(),
        };
        run.pending_initial = Some(common::wait_run_initial_segment(&run.lines, || {
            run.stderr()
        }));
        run
    }

    fn design(target: &str, name: &str, args: &[&str]) -> Run {
        Run::start(&repo().join(DESIGN), "design", target, name, args)
    }

    fn stderr(&self) -> String {
        self.stderr
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn send(&mut self, line: &str) {
        let stdin = self.stdin.as_mut().expect("stdin open");
        writeln!(stdin, "{line}").expect("write control line");
        stdin.flush().expect("flush control line");
    }

    /// The next record, within [`TIMEOUT`].
    fn next(&mut self) -> Json {
        match self
            .pending_initial
            .take()
            .map(Ok)
            .unwrap_or_else(|| self.lines.recv_timeout(TIMEOUT))
        {
            Ok(line) => {
                let json = Json::parse(&line).unwrap_or_else(|e| panic!("{line}: {e}"));
                self.seen.push(json.clone());
                json
            }
            Err(RecvTimeoutError::Timeout) => panic!(
                "no record within {TIMEOUT:?}; seen {} records, the last {:?}\nstderr:\n{}",
                self.seen.len(),
                self.seen.last().map(Json::compact),
                self.stderr()
            ),
            Err(RecvTimeoutError::Disconnected) => panic!(
                "ignite run ended; seen {} records\nstderr:\n{}",
                self.seen.len(),
                self.stderr()
            ),
        }
    }

    /// Consume records until one satisfies `predicate`; that record.
    fn wait_for(&mut self, what: &str, predicate: impl Fn(&Json) -> bool) -> Json {
        let started = Instant::now();
        loop {
            assert!(
                started.elapsed() < TIMEOUT,
                "no {what} within {TIMEOUT:?}\nstderr:\n{}",
                self.stderr()
            );
            let record = self.next();
            if predicate(&record) {
                return record;
            }
        }
    }

    fn wait_event(&mut self, variant: &str) -> Json {
        self.wait_for(&format!("event {variant}"), |r| {
            event_variant(r) == Some(variant)
        })
    }

    fn wait_effect(&mut self, variant: &str) -> Json {
        self.wait_for(&format!("effect {variant}"), |r| {
            kind(r) == "effect" && effect_variant(r) == Some(variant)
        })
    }

    fn wait_note(&mut self, contains: &str) -> Json {
        self.wait_for(&format!("note `{contains}`"), |r| {
            note_text(r).is_some_and(|t| t.contains(contains))
        })
    }

    fn wait_kind(&mut self, record_kind: &str) -> Json {
        self.wait_for(record_kind, |r| kind(r) == record_kind)
    }

    /// `status` and its line.
    fn status(&mut self) -> Json {
        self.send("status");
        self.wait_for("status", |r| kind(r) == "status")
    }

    /// `stop`, then the exit status and the remaining records.
    fn stop(&mut self) -> i32 {
        self.send("stop");
        self.finish()
    }

    /// Wait for the exit (the run was asked to stop or ends on its own).
    fn finish(&mut self) -> i32 {
        let started = Instant::now();
        let status = loop {
            if let Some(status) = self.child.try_wait().expect("wait ignite run") {
                break status;
            }
            assert!(
                started.elapsed() < TIMEOUT,
                "ignite run did not exit within {TIMEOUT:?}\nstderr:\n{}",
                self.stderr()
            );
            std::thread::sleep(Duration::from_millis(10));
        };
        while let Ok(line) = self.lines.try_recv() {
            self.seen
                .push(Json::parse(&line).unwrap_or_else(|e| panic!("{line}: {e}")));
        }
        status.code().unwrap_or(-1)
    }

    /// Item 7: the journal replays, exit 0, and the standings are printed.
    fn replay(&self) -> String {
        replay(&self.file, &self.state, &self.extra)
    }

    /// The pids of every `Started` event seen.
    /// The pids of every start, read from the supervisors' `Started`
    /// events (a rollout and a scheduler see the same event).
    fn pids(&self) -> Vec<i64> {
        self.seen
            .iter()
            .filter(|r| event_variant(r) == Some("Started") && machine(r).ends_with(".supervisor"))
            .filter_map(|r| event_fields(r).first().and_then(Json::as_i64))
            .collect()
    }

    fn journal_lines(&self) -> Vec<String> {
        std::fs::read_to_string(self.state.join("journal.jsonl"))
            .expect("read the journal")
            .lines()
            .map(str::to_string)
            .collect()
    }
}

impl Drop for Run {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// `ignite replay` of `state`: exit 0 and the standings on stdout.
fn replay(file: &Path, state: &Path, extra: &[String]) -> String {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ignite"));
    command
        .arg("replay")
        .arg(file)
        .args(["--assembly", "local", "--state"])
        .arg(state)
        .current_dir(repo());
    if let Some(index) = extra.iter().position(|a| a == "--fuel") {
        command.args(["--fuel", &extra[index + 1]]);
    }
    let output = command.output().expect("run ignite replay");
    assert_eq!(
        output.status.code(),
        Some(0),
        "replay of {}:\n{}",
        state.display(),
        text(&output.stderr)
    );
    text(&output.stdout)
}

/// No process the run started is left (16.0 item 1), checked through
/// `/proc`; the kill of a stopped child is asynchronous, so wait a little.
fn assert_no_process_left(pids: &[i64]) {
    for pid in pids {
        let proc_dir = PathBuf::from(format!("/proc/{pid}"));
        let started = Instant::now();
        while proc_dir.exists() && started.elapsed() < Duration::from_secs(2) {
            let status = std::fs::read_to_string(proc_dir.join("status")).unwrap_or_default();
            if status
                .lines()
                .any(|l| l.starts_with("State:") && l.contains('Z'))
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let status = std::fs::read_to_string(proc_dir.join("status")).unwrap_or_default();
        let zombie = status
            .lines()
            .any(|l| l.starts_with("State:") && l.contains('Z'));
        assert!(
            !proc_dir.exists() || zombie,
            "pid {pid} is still alive:\n{status}"
        );
    }
}

fn kind(record: &Json) -> &str {
    record.get("kind").and_then(Json::as_str).unwrap_or("")
}

fn at_ns(record: &Json) -> i64 {
    record
        .get("at")
        .and_then(|at| at.get("ns"))
        .and_then(Json::as_i64)
        .unwrap_or_else(|| panic!("no at in {}", record.compact()))
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

/// A field of a record value's `fields` object (a `Standing`, a
/// `Sample`, a `ProbeAttempt`).
fn field<'a>(value: &'a Json, name: &str) -> Option<&'a Json> {
    value.get("fields")?.get(name)
}

/// Whether the prompt arrived, read from a `std.sli.Sample` (`docs/slice.md`
/// 22.3): `Some(false)` for a sample without a line (the deadline's), else
/// the `received` field of the `ProbeAttempt` its line encodes.
fn sample_received(sample: &Json) -> Option<bool> {
    let line = field(sample, "line")?;
    match line.get("variant")?.as_str()? {
        "None" => Some(false),
        "Some" => {
            let text = line.get("fields")?.as_array()?.first()?.as_str()?;
            field(&Json::parse(text).ok()?, "received")?.as_bool()
        }
        _ => None,
    }
}

fn standing_status(record: &Json) -> &str {
    record
        .get("value")
        .and_then(|v| field(v, "status"))
        .and_then(|s| s.get("variant"))
        .and_then(Json::as_str)
        .unwrap_or("")
}

fn standing_int(record: &Json, name: &str) -> i64 {
    record
        .get("value")
        .and_then(|v| field(v, name))
        .and_then(Json::as_i64)
        .unwrap_or_else(|| panic!("no {name} in {}", record.compact()))
}

fn standing_contract(record: &Json) -> &str {
    record
        .get("value")
        .and_then(|v| field(v, "contract"))
        .and_then(Json::as_str)
        .unwrap_or("")
}

/// A TCP client of the terminal: reads the prompt, `echo hello` answers
/// `hello` (16.0 item 1).
fn echo_hello(port: i64) {
    let mut stream = TcpStream::connect(("127.0.0.1", port as u16)).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("read timeout");
    let mut seen = Vec::new();
    let mut buffer = [0u8; 256];
    while !seen.ends_with(PROMPT) {
        let n = stream.read(&mut buffer).expect("read prompt");
        assert!(n > 0, "eof before the prompt: {}", text(&seen));
        seen.extend_from_slice(&buffer[..n]);
    }
    stream.write_all(b"echo hello\r\n").expect("write echo");
    seen.clear();
    while !text(&seen).contains("hello") {
        let n = stream.read(&mut buffer).expect("read echo");
        assert!(n > 0, "eof before the echo: {}", text(&seen));
        seen.extend_from_slice(&buffer[..n]);
    }
    assert!(text(&seen).contains("hello"), "{}", text(&seen));
    stream.write_all(b"quit\r\n").ok();
}

/// 16.0 item 1 on one target: build, start, `started` then `ready`, a
/// client echo on the reported port, `client` declared, `stop` exits 0
/// and leaves no process; then item 7 (replay) and item 4 (no `standing`
/// claims `Compliant` without an attempt).
/// Row 5 of `docs/slice.md` 22.0: the journal is `ignite-journal/0.2`,
/// every `signal` record's value is a `std.sli.Sample` and every `Emitted`
/// event carries a `std.sli.Verdict`; the journal holds at least one of
/// each.
fn assert_typed_payloads(lines: &[String]) {
    let header = Json::parse(&lines[0]).unwrap();
    assert_eq!(
        header.get("formatVersion").and_then(Json::as_str),
        Some("ignite-journal/0.2"),
        "{}",
        lines[0]
    );
    let type_of = |value: Option<&Json>| {
        value
            .and_then(|v| v.get("type"))
            .and_then(Json::as_str)
            .map(str::to_string)
    };
    let (mut signals, mut emitted) = (0, 0);
    for line in &lines[1..] {
        let record = Json::parse(line).unwrap();
        if kind(&record) == "signal" {
            signals += 1;
            let value = record.get("value").and_then(|v| v.get("value"));
            assert_eq!(type_of(value).as_deref(), Some("std.sli.Sample"), "{line}");
        }
        if event_variant(&record) == Some("Emitted") {
            emitted += 1;
            let verdict = event_fields(&record).get(1);
            assert_eq!(
                type_of(verdict).as_deref(),
                Some("std.sli.Verdict"),
                "{line}"
            );
        }
    }
    assert!(
        signals > 0 && emitted > 0,
        "{signals} signals, {emitted} Emitted"
    );
}

fn terminal_runs(target: &str) {
    let mut run = Run::design(target, &format!("item1-{target}"), &["--clock", "manual"]);
    let segment = run.wait_kind("segment");
    let declared: Vec<&str> = segment
        .get("value")
        .and_then(|v| field(v, "declared"))
        .and_then(Json::as_array)
        .unwrap_or(&[])
        .iter()
        .filter_map(Json::as_str)
        .collect();
    assert_eq!(declared, ["Terminal.client"], "{}", segment.compact());
    let started = run.wait_event("Started");
    assert_eq!(machine(&started), "Terminal.server.supervisor");
    let ready = run.wait_event("Ready");
    assert_eq!(machine(&ready), "Terminal.server.supervisor");
    assert_eq!(at_ns(&ready), 0, "manual time has not moved");
    let status = run.status();
    let port = status
        .get("ports")
        .and_then(|p| p.get("Terminal.ingress"))
        .and_then(Json::as_i64)
        .unwrap_or_else(|| panic!("no port in {}", status.compact()));
    assert!(port != 2323 && port > 0, "port {port}");
    let declared: Vec<&str> = status
        .get("declared")
        .and_then(Json::as_array)
        .unwrap_or(&[])
        .iter()
        .filter_map(Json::as_str)
        .collect();
    assert_eq!(declared, ["Terminal.client"]);
    echo_hello(port);
    let reachable = run.wait_for("standing of Terminal.reachable", |r| {
        kind(r) == "standing" && standing_contract(r) == "Terminal.reachable"
    });
    assert_eq!(
        standing_status(&reachable),
        "Compliant",
        "{}",
        reachable.compact()
    );
    let pids = run.pids();
    assert_eq!(pids.len(), 1, "{pids:?}");
    let code = run.stop();
    assert_eq!(code, 0, "stderr:\n{}", run.stderr());
    assert_no_process_left(&pids);
    assert_typed_payloads(&run.journal_lines());
    let stderr = run.stderr();
    assert!(
        stderr.contains("Terminal.client: declared, not buildable: its kind has no main"),
        "{stderr}"
    );
    let first_signal = run
        .seen
        .iter()
        .find(|r| kind(r) == "signal")
        .map(|r| r.get("seq").and_then(Json::as_i64).unwrap_or(0))
        .unwrap_or(i64::MAX);
    for record in &run.seen {
        if kind(record) == "standing" {
            let seq = record.get("seq").and_then(Json::as_i64).unwrap_or(0);
            assert!(
                seq > first_signal,
                "a standing before the first attempt: {}",
                record.compact()
            );
        }
    }
    let standings = run.replay();
    assert!(standings.contains("Terminal.reachable"), "{standings}");
}

/// 16.0 item 4, end to end: a run stopped before the first slot. The
/// listener's port is held by the test, so `terminald` exits at once and
/// never becomes ready; no attempt runs, no `standing` or `signal` is
/// journaled, and `status` prints every contract as
/// `InsufficientEvidence` with no attempt executed.
#[test]
fn zero_attempts_give_insufficient_evidence() {
    let held = std::net::TcpListener::bind("127.0.0.1:0").expect("bind a port to hold");
    let port = held.local_addr().expect("local addr").port();
    let port_arg = format!("Terminal.ingress={port}");
    let mut run = Run::design("rust", "item4", &["--clock", "manual", "--port", &port_arg]);
    let exited = run.wait_for("Exited of Terminal.server", |r| {
        event_variant(r) == Some("Exited") && machine(r) == "Terminal.server.supervisor"
    });
    assert_ne!(
        event_fields(&exited)[1].as_i64(),
        Some(0),
        "{}",
        exited.compact()
    );
    run.wait_note("restart attempt 1 delay 100 ms");
    let status = run.status();
    let standings = status
        .get("standings")
        .and_then(Json::as_array)
        .unwrap_or_else(|| panic!("{}", status.compact()));
    let mut contracts: Vec<&str> = standings
        .iter()
        .filter_map(|s| field(s, "contract").and_then(Json::as_str))
        .collect();
    contracts.sort_unstable();
    assert_eq!(
        contracts,
        [
            "Terminal.customerAccess",
            "Terminal.promptFast",
            "Terminal.reachable"
        ],
        "{}",
        status.compact()
    );
    for standing in standings {
        let status_of = field(standing, "status")
            .and_then(|s| s.get("variant"))
            .and_then(Json::as_str);
        assert_eq!(
            status_of,
            Some("InsufficientEvidence"),
            "{}",
            standing.compact()
        );
        assert_eq!(field(standing, "executed").and_then(Json::as_i64), Some(0));
        assert_eq!(field(standing, "good").and_then(Json::as_i64), Some(0));
    }
    let pids = run.pids();
    let code = run.stop();
    assert_eq!(code, 0, "stderr:\n{}", run.stderr());
    assert_no_process_left(&pids);
    assert!(
        run.seen
            .iter()
            .all(|r| kind(r) != "standing" && kind(r) != "signal"),
        "a standing or signal without an attempt"
    );
    run.replay();
    drop(held);
}

#[test]
fn terminal_runs_under_ignite_run() {
    terminal_runs("rust");
}

#[test]
fn terminal_runs_under_ignite_run_on_the_llvm_target() {
    terminal_runs("llvm");
}

/// 16.0 item 2, manual clock: SIGKILL to `terminald`, `restart attempt 1
/// delay 100 ms`, nothing at `advance 99 ms`, `started` at exactly
/// `ended.at + 100 ms` after `advance 1 ms`, `delay 200 ms` after a
/// second kill.
#[test]
fn killing_terminald_restarts_it_within_backoff() {
    let mut run = Run::design("rust", "item2-manual", &["--clock", "manual"]);
    run.wait_event("Ready");
    let pid = run.pids()[0];
    send_signal(pid, SIGKILL);
    let ended = run.wait_for("Exited or Signalled", |r| {
        matches!(event_variant(r), Some("Exited" | "Signalled"))
            && machine(r) == "Terminal.server.supervisor"
    });
    assert_eq!(event_fields(&ended)[0].as_i64(), Some(pid));
    run.wait_note("restart attempt 1 delay 100 ms");
    let timer = run.wait_effect("SetTimer");
    assert_eq!(
        effect_fields(&timer)[1].get("ns").and_then(Json::as_i64),
        Some(at_ns(&ended) + 100 * NS_PER_MS)
    );
    run.send("advance 99 ms");
    let before = run.seen.len();
    run.status();
    assert!(
        run.seen[before..]
            .iter()
            .all(|r| event_variant(r) != Some("Started")),
        "a start before the backoff elapsed"
    );
    run.send("advance 1 ms");
    let started = run.wait_event("Started");
    assert_eq!(at_ns(&started), at_ns(&ended) + 100 * NS_PER_MS);
    run.wait_event("Ready");
    let pid = *run.pids().last().unwrap();
    send_signal(pid, SIGKILL);
    run.wait_note("restart attempt 2 delay 200 ms");
    let pids = run.pids();
    let code = run.stop();
    assert_eq!(code, 0, "stderr:\n{}", run.stderr());
    assert_no_process_left(&pids);
    run.replay();
}

/// 16.0 item 2, real clock: `started.at - ended.at` lies in [100 ms,
/// 500 ms].
#[test]
fn killing_terminald_restarts_it_in_real_time() {
    let mut run = Run::design("rust", "item2-real", &["--clock", "real"]);
    run.wait_event("Ready");
    let pid = run.pids()[0];
    send_signal(pid, SIGKILL);
    let ended = run.wait_for("Exited or Signalled", |r| {
        matches!(event_variant(r), Some("Exited" | "Signalled"))
    });
    let started = run.wait_event("Started");
    let delay = at_ns(&started) - at_ns(&ended);
    assert!(
        (100 * NS_PER_MS..=500 * NS_PER_MS).contains(&delay),
        "restart after {delay} ns"
    );
    run.wait_event("Ready");
    let pids = run.pids();
    let code = run.stop();
    assert_eq!(code, 0, "stderr:\n{}", run.stderr());
    assert_no_process_left(&pids);
    run.replay();
}

/// 16.0 item 2: `maximum_attempts = 2` journals `gave_up` on the third
/// failure inside `interval`; the run goes on and `stop` exits 1.
#[test]
fn exhausted_restarts_give_up_and_stop_exits_1() {
    let mut run = Run::start(
        &fixture("run_gave_up.ignite"),
        "gave-up",
        "rust",
        "gave-up",
        &["--clock", "manual"],
    );
    run.wait_note("restart attempt 1 delay 100 ms");
    run.send("advance 100 ms");
    run.wait_note("restart attempt 2 delay 100 ms");
    run.send("advance 100 ms");
    run.wait_note("gave_up");
    let status = run.status();
    assert!(
        status.compact().contains("\"variant\":\"Failed\""),
        "{}",
        status.compact()
    );
    let pids = run.pids();
    assert_eq!(pids.len(), 3, "{pids:?}");
    let code = run.stop();
    assert_eq!(code, 1, "stderr:\n{}", run.stderr());
    run.replay();
}

/// 16.0 items 3 and 5, manual clock from 30 s before the Europe/Berlin
/// month boundary of 2026-10-31T23:00Z: one good attempt; SIGSTOP makes
/// the next attempt hang, and it ends as a bad attempt either way: the
/// probe's own `timeout` (2 s real time, journaled at the clock's 10 s if
/// it lands in a settle wait) or the scheduler's deadline at 12 s; the
/// standing of `Terminal.reachable` reads `Violated` with `good 1` and
/// `executed 2`, and `status` prints it; after SIGCONT attempts are good
/// again and the 30-day window keeps `Violated`; the period close
/// publishes the closed standing and fires `access_commitment_breached`
/// once. The one test that pays a settle wait (16.4, A2).
#[test]
fn probe_timeout_is_visible_in_journal_and_standing() {
    let mut run = Run::design(
        "rust",
        "item3",
        &["--clock", "manual", "--epoch", "2026-10-31T22:59:30Z"],
    );
    run.wait_event("Ready");
    let first = run.wait_for("standing of Terminal.reachable", |r| {
        kind(r) == "standing" && standing_contract(r) == "Terminal.reachable"
    });
    assert_eq!(standing_int(&first, "good"), 1, "{}", first.compact());
    assert_eq!(standing_int(&first, "executed"), 1);
    let pid = run.pids()[0];
    send_signal(pid, SIGSTOP);
    run.send("advance 10 seconds");
    let attempt = run.wait_effect("Attempt");
    assert_eq!(at_ns(&attempt), 10_000 * NS_PER_MS);
    run.send("advance 2 seconds");
    let bad = run.wait_for("bad attempt signal", |r| {
        kind(r) == "signal"
            && r.get("value")
                .and_then(|v| v.get("value"))
                .and_then(sample_received)
                == Some(false)
    });
    let bad_at = at_ns(&bad);
    assert!(
        bad_at == 10_000 * NS_PER_MS || bad_at == 12_000 * NS_PER_MS,
        "{}",
        bad.compact()
    );
    let violated = run.wait_for("standing of Terminal.reachable", |r| {
        kind(r) == "standing" && standing_contract(r) == "Terminal.reachable"
    });
    assert_eq!(
        standing_status(&violated),
        "Violated",
        "{}",
        violated.compact()
    );
    assert_eq!(standing_int(&violated, "good"), 1);
    assert_eq!(standing_int(&violated, "executed"), 2);
    let timed_out = run.seen.iter().any(|r| {
        note_text(r) == Some("probe timeout")
            || (event_variant(r) == Some("AttemptDone")
                && event_fields(r)
                    .get(1)
                    .and_then(Json::as_str)
                    .and_then(|line| Json::parse(line).ok())
                    .and_then(|a| field(&a, "received").and_then(Json::as_bool))
                    == Some(false))
    });
    assert!(timed_out, "neither a probe timeout nor a bad AttemptDone");
    let status = run.status();
    let standings = status.get("standings").and_then(Json::as_array).unwrap();
    let reachable = standings
        .iter()
        .find(|s| field(s, "contract").and_then(Json::as_str) == Some("Terminal.reachable"))
        .unwrap_or_else(|| panic!("{}", status.compact()));
    assert_eq!(
        reachable.compact(),
        violated.get("value").unwrap().compact(),
        "status differs from the journal"
    );
    send_signal(pid, SIGCONT);
    run.send("advance 10 seconds");
    let good = run.wait_for("good attempt signal", |r| {
        kind(r) == "signal"
            && at_ns(r) == 20_000 * NS_PER_MS
            && r.get("value")
                .and_then(|v| v.get("value"))
                .and_then(sample_received)
                == Some(true)
    });
    assert_eq!(at_ns(&good), 20_000 * NS_PER_MS);
    let still = run.wait_for("standing of Terminal.reachable", |r| {
        kind(r) == "standing"
            && standing_contract(r) == "Terminal.reachable"
            && at_ns(r) == 20_000 * NS_PER_MS
    });
    assert_eq!(standing_status(&still), "Violated");
    assert_eq!(standing_int(&still, "executed"), 3);
    run.send("advance 10 seconds");
    let closed = run.wait_for("closed period of Terminal.customerAccess", |r| {
        kind(r) == "standing"
            && standing_contract(r) == "Terminal.customerAccess"
            && at_ns(r) == 30_000 * NS_PER_MS
    });
    assert_eq!(standing_status(&closed), "Violated", "{}", closed.compact());
    let consequence = run.wait_kind("consequence");
    let fired = consequence.get("value").unwrap();
    assert_eq!(
        field(fired, "consequence").and_then(Json::as_str),
        Some("access_commitment_breached")
    );
    assert_eq!(
        field(fired, "standing_seq").and_then(Json::as_i64),
        closed.get("seq").and_then(Json::as_i64)
    );
    let pids = run.pids();
    let code = run.stop();
    assert_eq!(code, 0, "stderr:\n{}", run.stderr());
    assert_no_process_left(&pids);
    assert_eq!(
        run.seen.iter().filter(|r| kind(r) == "consequence").count(),
        1,
        "one consequence per period"
    );
    let standings = run.replay();
    assert!(standings.contains("Terminal.reachable"), "{standings}");
}

/// 16.0 item 5: a run with only good attempts crosses the same boundary
/// and journals no consequence.
#[test]
fn good_attempts_across_the_period_fire_no_consequence() {
    let mut run = Run::design(
        "rust",
        "item5-good",
        &["--clock", "manual", "--epoch", "2026-10-31T22:59:30Z"],
    );
    run.wait_event("Ready");
    run.send("advance 30 seconds");
    let closed = run.wait_for("closed period of Terminal.customerAccess", |r| {
        kind(r) == "standing"
            && standing_contract(r) == "Terminal.customerAccess"
            && at_ns(r) == 30_000 * NS_PER_MS
    });
    assert_eq!(
        standing_status(&closed),
        "Compliant",
        "{}",
        closed.compact()
    );
    assert_eq!(standing_int(&closed, "good"), 3);
    assert_eq!(standing_int(&closed, "executed"), 3);
    let pids = run.pids();
    let code = run.stop();
    assert_eq!(code, 0, "stderr:\n{}", run.stderr());
    assert_no_process_left(&pids);
    assert!(
        run.seen.iter().all(|r| kind(r) != "consequence"),
        "a consequence fired"
    );
    run.replay();
}

/// Mask the values a real process contributes (16.0 item 8): the pid of
/// lifecycle events and kills, the latency of attempts and emissions
/// (inside the probe's line, which `AttemptDone` and a `Sample` carry as
/// text since `docs/slice.md` 22.3).
fn masked(line: &str) -> String {
    fn walk(json: &mut Json) {
        match json {
            Json::Object(object) => {
                let variant = object
                    .get("variant")
                    .and_then(Json::as_str)
                    .map(str::to_string);
                if let Some(Json::Array(fields)) = object.get_mut("fields") {
                    match variant.as_deref() {
                        Some("Started" | "Ready" | "Exited" | "Signalled") => {
                            if let Some(pid) = fields.first_mut() {
                                *pid = Json::Int(0);
                            }
                        }
                        Some("Kill") => {
                            if let Some(pid) = fields.get_mut(1) {
                                *pid = Json::Int(0);
                            }
                        }
                        _ => {}
                    }
                }
                if let Some(Json::Object(fields)) = object.get_mut("fields")
                    && fields.contains_key("received")
                    && let Some(latency) = fields.get_mut("latency")
                {
                    *latency = Json::Int(0);
                }
                for value in object.values_mut() {
                    walk(value);
                }
            }
            Json::Array(items) => items.iter_mut().for_each(walk),
            Json::Str(text) if text.starts_with('{') => {
                if let Ok(mut inner) = Json::parse(text) {
                    walk(&mut inner);
                    *text = inner.compact();
                }
            }
            _ => {}
        }
    }
    let mut json = Json::parse(line).unwrap();
    walk(&mut json);
    json.compact()
}

/// 16.0 item 8: two manual-clock runs of one scenario give equal journals
/// after masking `pid` and `latency`.
#[test]
fn two_manual_runs_give_equal_journals_after_masking() {
    let scenario = |name: &str| {
        let mut run = Run::design("rust", name, &["--clock", "manual"]);
        run.wait_event("Ready");
        run.wait_for("standing of Terminal.reachable", |r| {
            kind(r) == "standing" && standing_contract(r) == "Terminal.reachable"
        });
        run.send("advance 10 seconds");
        run.wait_for("second standing of Terminal.reachable", |r| {
            kind(r) == "standing"
                && standing_contract(r) == "Terminal.reachable"
                && at_ns(r) == 10_000 * NS_PER_MS
        });
        let code = run.stop();
        assert_eq!(code, 0, "stderr:\n{}", run.stderr());
        run.replay();
        let lines = run.journal_lines();
        assert!(
            lines
                .iter()
                .map(|line| Json::parse(line).unwrap())
                .any(|record| {
                    machine(&record) == "Terminal.server.supervisor"
                        && matches!(event_variant(&record), Some("Exited" | "Signalled"))
                }),
            "the child's completion must be journaled before the closing segment"
        );
        lines
    };
    let first = scenario("item8-a");
    let second = scenario("item8-b");
    assert_eq!(first.len(), second.len(), "{first:#?}\n{second:#?}");
    for (a, b) in first.iter().zip(&second) {
        assert_eq!(masked(a), masked(b));
    }
}

fn run_fixture_diagnostic(name: &str, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ignite"));
    command.arg("run").arg(name).args(args);
    command
        .current_dir(fixtures_dir())
        .stdin(Stdio::null())
        .output()
        .expect("run ignite run")
}

fn compare(path: &Path, actual: &str) {
    if std::env::var_os("IGNITE_BLESS").is_some() {
        std::fs::write(path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{}: {e} (IGNITE_BLESS=1 to create)", path.display()));
    assert!(
        expected == actual,
        "{} differs from the output (IGNITE_BLESS=1 to accept):\n{actual}",
        path.display()
    );
}

/// E125 (16.17): `ignite run` on a program without an assembly exits 2.
#[test]
fn e125_nothing_to_run() {
    let output = run_fixture_diagnostic(
        "e125_nothing_to_run.ignite",
        &["--state", state("e125").to_str().unwrap()],
    );
    assert_eq!(output.status.code(), Some(2), "{}", text(&output.stderr));
    assert!(output.stdout.is_empty());
    compare(
        &fixture("e125_nothing_to_run.expected"),
        &text(&output.stderr),
    );
}

/// E126 (16.17, `docs/slice.md` 22.0 row 6): a state directory whose journal
/// has `formatVersion` `ignite-journal/0.1` (and another model), refused by
/// `ignite run` before anything is built (exit 2) and by `ignite replay`
/// (exit 1, its diagnostics exit like `plan`'s).
#[test]
fn e126_journal_mismatch() {
    let state = state("e126");
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(
        state.join("journal.jsonl"),
        "{\"epoch\":{\"ns\":0,\"type\":\"Duration\"},\"formatVersion\":\"ignite-journal/0.1\",\"kind\":\"header\",\"model\":{\"systemId\":\"Other\"},\"seq\":0}\n",
    )
    .unwrap();
    let output = run_fixture_diagnostic(
        "e126_journal_mismatch.ignite",
        &["--state", state.to_str().unwrap(), "--clock", "manual"],
    );
    assert_eq!(output.status.code(), Some(2), "{}", text(&output.stderr));
    assert!(output.stdout.is_empty());
    compare(
        &fixture("e126_journal_mismatch.expected"),
        &text(&output.stderr),
    );
    assert!(!state.join("build").exists(), "E126 cost a build");
    let replayed = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .args(["replay", "e126_journal_mismatch.ignite", "--state"])
        .arg(&state)
        .current_dir(fixtures_dir())
        .stdin(Stdio::null())
        .output()
        .expect("run ignite replay");
    assert_eq!(
        replayed.status.code(),
        Some(1),
        "{}",
        text(&replayed.stderr)
    );
    assert!(replayed.stdout.is_empty());
    compare(
        &fixture("e126_journal_mismatch.expected"),
        &text(&replayed.stderr),
    );
}

/// E127 (16.17, item 7): a journal with one `effect` record altered does
/// not replay; the diagnostic names its `seq`.
#[test]
fn e127_replay_diverged() {
    let mut run = Run::start(
        &fixture("e127_replay_diverged.ignite"),
        "e127",
        "rust",
        "e127",
        &["--clock", "manual"],
    );
    run.wait_note("restart attempt 1 delay 100 ms");
    let code = run.stop();
    assert_eq!(code, 0, "stderr:\n{}", run.stderr());
    run.replay();
    let path = run.state.join("journal.jsonl");
    let lines = run.journal_lines();
    let spawn = lines
        .iter()
        .position(|l| l.contains("\"kind\":\"effect\"") && l.contains("\"variant\":\"Spawn\""))
        .expect("a Spawn effect");
    assert_eq!(spawn, 3, "{lines:#?}");
    let mut altered = lines.clone();
    altered[spawn] = altered[spawn].replace("[\"S.daemon\",1]", "[\"S.daemon\",2]");
    assert_ne!(altered[spawn], lines[spawn]);
    std::fs::write(&path, altered.join("\n") + "\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .args(["replay", "e127_replay_diverged.ignite", "--state"])
        .arg(&run.state)
        .current_dir(fixtures_dir())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{}", text(&output.stderr));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E127]") && stderr.contains("journal record 3 does not replay"),
        "{stderr}"
    );
    compare(&fixture("e127_replay_diverged.expected"), &stderr);
}

/// E128 (16.17): with `--fuel 1` the first step fails; the run journals
/// an `error` record, goes on and exits 1 at `stop`.
#[test]
fn e128_reducer_failed() {
    let mut run = Run::start(
        &fixture("e128_reducer_failed.ignite"),
        "e128",
        "rust",
        "e128",
        &["--clock", "manual", "--fuel", "1"],
    );
    let error = run.wait_kind("error");
    assert_eq!(machine(&error), "S.daemon.supervisor");
    let status = run.status();
    assert_eq!(kind(&status), "status");
    let code = run.stop();
    assert_eq!(code, 1, "stderr:\n{}", run.stderr());
    compare(
        &fixture("e128_reducer_failed.expected"),
        &(error.compact() + "\n"),
    );
    run.replay();
}

/// The `--port` override is honoured and the free port of `0` is pinned
/// for the restart (16.3), seen through `status`.
#[test]
fn pinned_port_survives_a_restart() {
    let mut run = Run::design("rust", "pinned", &["--clock", "manual"]);
    run.wait_event("Ready");
    let before = run.status();
    let port = before
        .get("ports")
        .and_then(|p| p.get("Terminal.ingress"))
        .and_then(Json::as_i64)
        .unwrap();
    send_signal(run.pids()[0], SIGKILL);
    run.wait_note("restart attempt 1");
    run.send("advance 100 ms");
    run.wait_for("second Ready", |r| {
        event_variant(r) == Some("Ready") && at_ns(r) > 0
    });
    let after = run.status();
    assert_eq!(
        after
            .get("ports")
            .and_then(|p| p.get("Terminal.ingress"))
            .and_then(Json::as_i64),
        Some(port)
    );
    echo_hello(port);
    let pids = run.pids();
    let code = run.stop();
    assert_eq!(code, 0, "stderr:\n{}", run.stderr());
    assert_no_process_left(&pids);
    run.replay();
}

// ---- slice 17 (U4): E142 and settings under ignite run ----

/// Row 26 of `docs/slice.md` 21.0: `ignite run` passes no setting values,
/// so a started member with a `Setting` without `default` is E142, exit 2,
/// before anything starts (no state directory).
#[test]
fn a_setting_without_default_is_e142() {
    let state = state("e142");
    let output = run_fixture_diagnostic(
        "slice17/e142_setting_without_default.ignite",
        &["--state", state.to_str().unwrap()],
    );
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(output.stdout.is_empty());
    assert_eq!(stderr.matches("E142").count(), 1, "{stderr}");
    assert!(stderr.contains("`Bench.tool`"), "{stderr}");
    assert!(!stderr.contains("Bench.mode"), "{stderr}");
    compare(
        &fixture("slice17/e142_setting_without_default.expected"),
        &stderr,
    );
    assert!(!state.exists(), "the run created its state");
}

/// Row 26: the studio with `default = "/bin/sh"` runs under `ignite run`
/// in a working directory of its own: its member reads the default, the
/// relative root is created there, `status` shows the member running,
/// `stop` exits 0, and the repository gains no file.
#[test]
fn studio_defaults_runs() {
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("run-work-studio-defaults");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    let file = fixture("slice17/studio_defaults.ignite");
    let mut run = Run::start_in(
        &work,
        &file,
        "studio_defaults",
        "llvm",
        "studio-defaults",
        &["--clock", "manual", "--port", "Studio.ingress=0"],
    );
    run.wait_event("Ready");
    let status = run.status();
    let supervisor = status
        .get("machines")
        .and_then(Json::as_array)
        .and_then(|machines| {
            machines
                .iter()
                .find(|m| m.get("id").and_then(Json::as_str) == Some("Studio.server.supervisor"))
        })
        .unwrap_or_else(|| panic!("no supervisor in {}", status.compact()))
        .compact();
    assert!(supervisor.contains("Running"), "{supervisor}");
    let port = status
        .get("ports")
        .and_then(|p| p.get("Studio.ingress"))
        .and_then(Json::as_i64)
        .unwrap_or_else(|| panic!("no port in {}", status.compact()));
    let get = |path: &str| {
        let mut stream = std::net::TcpStream::connect(("127.0.0.1", port as u16)).unwrap();
        stream.set_read_timeout(Some(TIMEOUT)).unwrap();
        write!(
            stream,
            "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
        let mut reply = String::new();
        stream.read_to_string(&mut reply).unwrap();
        reply
    };
    let tool = get("/tool");
    assert!(tool.starts_with("HTTP/1.1 200"), "{tool}");
    assert!(tool.ends_with("\r\n\r\n/bin/sh"), "{tool}");
    let canonical = std::fs::canonicalize(work.join("workspace")).expect("the root was created");
    let root = get("/root");
    assert!(
        root.ends_with(&format!("\r\n\r\n{}", canonical.display())),
        "{root}"
    );
    let pids = run.pids();
    let code = run.stop();
    assert_eq!(code, 0, "stderr:\n{}", run.stderr());
    assert_no_process_left(&pids);
    for place in [
        repo().join("workspace"),
        repo().join("crates/ignite/workspace"),
        fixtures_dir().join("workspace"),
        fixtures_dir().join("slice17/workspace"),
    ] {
        assert!(!place.exists(), "{} was created", place.display());
    }
}

/// 15.6 and 21.3: the studio under the standard `Deployment`, which binds
/// only its listener and server, runs: its capability resources are
/// hosted by the server's process without a binding, the member reads its
/// setting's default, and `stop` exits 0. The server is the standard
/// `HttpServer`, whose `machine` hook (16.16) is the supervisor the run
/// steps.
#[test]
fn studio_under_the_standard_deployment_runs() {
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("run-work-studio-deployment");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    let file = fixture("slice17/studio_deployment.ignite");
    let mut run = Run::start_in(
        &work,
        &file,
        "studio_deployment",
        "llvm",
        "studio-deployment",
        &["--clock", "manual", "--port", "Studio.ingress=0"],
    );
    run.wait_event("Ready");
    let status = run.status();
    let supervisor = status
        .get("machines")
        .and_then(Json::as_array)
        .and_then(|machines| {
            machines
                .iter()
                .find(|m| m.get("id").and_then(Json::as_str) == Some("Studio.server.supervisor"))
        })
        .unwrap_or_else(|| panic!("no supervisor in {}", status.compact()))
        .compact();
    assert!(supervisor.contains("Running"), "{supervisor}");
    let port = status
        .get("ports")
        .and_then(|p| p.get("Studio.ingress"))
        .and_then(Json::as_i64)
        .unwrap_or_else(|| panic!("no port in {}", status.compact()));
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port as u16)).unwrap();
    stream.set_read_timeout(Some(TIMEOUT)).unwrap();
    write!(
        stream,
        "GET /tool HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut reply = String::new();
    stream.read_to_string(&mut reply).unwrap();
    assert!(reply.starts_with("HTTP/1.1 200"), "{reply}");
    assert!(reply.ends_with("\r\n\r\n/bin/sh"), "{reply}");
    assert!(work.join("workspace").is_dir(), "the root was not created");
    let pids = run.pids();
    let code = run.stop();
    assert_eq!(code, 0, "stderr:\n{}", run.stderr());
    assert_no_process_left(&pids);
}

/// 16.3: with two `TcpListener`s in one member, `--port` is passed for the
/// first the member declares (`Duo.zeta`), which is the one the generated
/// `main` applies it to, although `Duo.alpha` sorts first.
#[test]
fn the_first_declared_listener_takes_the_port() {
    let port = std::net::TcpListener::bind(("127.0.0.1", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let pinned = format!("Duo.zeta={port}");
    let mut run = Run::start(
        &fixture("slice17/two_listeners.ignite"),
        "two_listeners",
        "llvm",
        "two-listeners",
        &["--clock", "manual", "--port", &pinned],
    );
    run.wait_event("Ready");
    let status = run.status();
    let ports = status
        .get("ports")
        .unwrap_or_else(|| panic!("no ports in {}", status.compact()))
        .compact();
    assert_eq!(ports, format!(r#"{{"Duo.zeta":{port}}}"#));
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream.set_read_timeout(Some(TIMEOUT)).unwrap();
    write!(
        stream,
        "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut reply = String::new();
    stream.read_to_string(&mut reply).unwrap();
    assert!(reply.ends_with("\r\n\r\nzeta"), "{reply}");
    let pids = run.pids();
    let code = run.stop();
    assert_eq!(code, 0, "stderr:\n{}", run.stderr());
    assert_no_process_left(&pids);
}

/// Adversary (U4), row 26 and 21.6: the started member reaches the
/// `Setting` without `default` only through a component it embeds; its
/// generated `main` still creates the setting, so `ignite run` refuses the
/// system with one E142 naming the setting and the started member.
#[test]
fn adversary_e142_through_an_embedded_component() {
    let state = state("adv-e142-embedded");
    let output = run_fixture_diagnostic(
        "slice17/adversary_e142_embedded.ignite",
        &["--state", state.to_str().unwrap()],
    );
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(output.stdout.is_empty());
    assert_eq!(stderr.matches("E142").count(), 1, "{stderr}");
    assert!(
        stderr.contains(
            "setting `Bench.tool` (`--tool`) has no `default`, and `Bench.runner` uses it"
        ),
        "{stderr}"
    );
    assert!(!state.exists(), "the run created its state");
}
