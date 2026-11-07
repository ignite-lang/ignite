//! The slice 18 fault matrix on the todo system (`docs/slice.md` 22.0 rows
//! 16 to 22, 22.8). Row 16 runs the todo under `ignite run` with the manual
//! clock on both targets and drives its `Availability` contract through
//! every status; rows 17 to 22 run it under `ignite host` and `ignite apply`
//! on the LLVM target, the slice's service target. Every process a test
//! starts runs in a working directory under the test's own scratch
//! directory, so the todo's relative `todo-data` never resolves inside the
//! repository.

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ignite_eval::Json;

use crate::common;
use crate::common::apply::{BUILD, LIVE};

/// Row 17's bound on the restart after a SIGKILL of the server, which the
/// test asserts and keeps; alone it takes 0.44 to 0.53 s (2 runs,
/// 2026-11-07). The other deadlines of this file are [`LIVE`],
/// [`BUILD`], [`STOP_ON_DROP`] and [`REMAINING`].
const RESTART: Duration = Duration::from_secs(10);
/// How long a `Host` dropped without `stop` gets to stop before it is
/// killed; cleanup, no assertion.
const STOP_ON_DROP: Duration = Duration::from_secs(10);
/// The longest gap between two records `Run::stop` drains after the run
/// exited. The drain ends when the reader closes the channel at the end of
/// stdout, so the ceiling only bounds a reader that lags; at the former
/// 1 s a reader delayed under load cut the records the standings
/// assertions read.
const REMAINING: Duration = Duration::from_secs(30);

const FAULT: &str = "IGNITE_DEBUG_HOST_FAULT";
const SERVER: &str = "TodoService.server";
const CONTRACT: &str = "TodoService.reachable";
const MILK: &str = r#"[{"id":1,"title":"milk","done":false}]"#;
const HEALTH: &str = r#"("GET", ["health"]) => Response.plain(200, "ok\n")"#;
const DAMAGED: &str = "restore host-journal.jsonl or move the state aside; the host does not start on a damaged journal";

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

fn wait(child: &mut Child, timeout: Duration) -> Option<ExitStatus> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().expect("read child exit") {
            return Some(status);
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Wait until `done`, at most `limit`; `context` explains a timeout.
fn until(
    what: &str,
    limit: Duration,
    mut done: impl FnMut() -> bool,
    context: impl Fn() -> String,
) {
    let deadline = Instant::now() + limit;
    while !done() {
        assert!(Instant::now() < deadline, "{what}: {}", context());
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn free_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn kill(pid: i64, signal: nix::sys::signal::Signal) {
    nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid as i32), signal)
        .unwrap_or_else(|error| panic!("signal {signal} to {pid}: {error}"));
}

/// A fresh directory for one test: `todo/` holds a copy of the todo
/// example with its listener on `port`, `work/` is the working directory
/// of every process the test starts, `state/` the run or host state.
struct Scratch {
    root: PathBuf,
    work: PathBuf,
    state: PathBuf,
    source: PathBuf,
    port: u16,
}

impl Scratch {
    fn new(label: &str) -> Scratch {
        let port = free_port();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("slice18-{label}-{}-{stamp}", std::process::id()));
        let todo = root.join("todo");
        let work = root.join("work");
        let state = root.join("state");
        for dir in [&todo, &work, &state] {
            fs::create_dir_all(dir).unwrap();
        }
        for entry in fs::read_dir(repo().join("examples/todo")).unwrap() {
            let path = entry.unwrap().path();
            if path
                .extension()
                .is_some_and(|extension| extension == "ignite")
            {
                fs::copy(&path, todo.join(path.file_name().unwrap())).unwrap();
            }
        }
        let source = todo.join("todo.ignite");
        let original = read(&source);
        assert!(original.contains("port = 8080"), "todo fixture port");
        fs::write(
            &source,
            original.replace("port = 8080", &format!("port = {port}")),
        )
        .unwrap();
        Scratch {
            root: fs::canonicalize(&root).unwrap(),
            work: fs::canonicalize(&work).unwrap(),
            state: fs::canonicalize(&state).unwrap(),
            source: fs::canonicalize(&source).unwrap(),
            port,
        }
    }

    /// Replace `before` in the copy's `todo.ignite` with `after`.
    fn edit(&self, before: &str, after: &str) {
        let original = read(&self.source);
        assert!(original.contains(before), "todo copy holds {before}");
        fs::write(&self.source, original.replacen(before, after, 1)).unwrap();
    }
}

/// One HTTP/1.1 request on its own connection: status and body.
fn http(port: u16, method: &str, path: &str, body: Option<&str>) -> (u16, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", port))
        .unwrap_or_else(|error| panic!("connect to todod on {port}: {error}"));
    stream.set_read_timeout(Some(LIVE)).unwrap();
    let body = body.unwrap_or("");
    let mut request =
        format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n");
    if !body.is_empty() {
        request.push_str(&format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\n",
            body.len()
        ));
    }
    request.push_str("\r\n");
    request.push_str(body);
    stream.write_all(request.as_bytes()).unwrap();
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 65536];
    let head_end = loop {
        if let Some(at) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
            break at + 4;
        }
        let n = stream.read(&mut chunk).expect("read from todod");
        assert!(n > 0, "todod closed before a whole head: {buffer:?}");
        buffer.extend_from_slice(&chunk[..n]);
    };
    let head = String::from_utf8_lossy(&buffer[..head_end - 4]).into_owned();
    let mut lines = head.split("\r\n");
    let status_line = lines.next().unwrap();
    let status = status_line
        .strip_prefix("HTTP/1.1 ")
        .and_then(|rest| rest.get(..3))
        .and_then(|code| code.parse().ok())
        .unwrap_or_else(|| panic!("status line {status_line:?}"));
    let length: usize = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .map(|(_, value)| value.trim().parse().expect("Content-Length"))
        .unwrap_or(0);
    while buffer.len() < head_end + length {
        let n = stream.read(&mut chunk).expect("read a body from todod");
        assert!(n > 0, "todod closed inside a body");
        buffer.extend_from_slice(&chunk[..n]);
    }
    let body = String::from_utf8_lossy(&buffer[head_end..head_end + length]).into_owned();
    (status, body)
}

// ---- row 16: the contract under `ignite run` ----------------------------------

/// A running `ignite run` of the todo copy with its `--json` records.
struct Run {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Receiver<String>,
    stderr: Arc<Mutex<String>>,
    seen: Vec<Json>,
}

impl Run {
    fn start(scratch: &Scratch, target: &str) -> Run {
        let mut command = Command::new(env!("CARGO_BIN_EXE_ignite"));
        command
            .arg("run")
            .arg(&scratch.source)
            .args(["--assembly", "local", "--state"])
            .arg(&scratch.state)
            .arg("--build")
            .arg(Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("slice18-run-build-{target}")))
            .args([
                "--target", target, "--json", "--settle", "1 s", "--clock", "manual",
            ])
            .arg("--port")
            .arg(format!("TodoService.ingress={}", scratch.port));
        if target == "llvm" {
            command.arg("--runtime").arg(common::runtime_archive());
        } else {
            command
                .arg("--target-dir")
                .arg(Path::new(env!("CARGO_TARGET_TMPDIR")).join("run-cargo"));
        }
        command
            .current_dir(&scratch.work)
            .env("CARGO_INCREMENTAL", "0")
            .env("CARGO_PROFILE_DEV_DEBUG", "line-tables-only")
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
                        .push_str(&String::from_utf8_lossy(&buffer[..n])),
                }
            }
        });
        Run {
            child,
            stdin,
            lines,
            stderr,
            seen: Vec::new(),
        }
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

    /// Consume records until one satisfies `predicate`, at most `limit`.
    fn wait_for(&mut self, what: &str, limit: Duration, predicate: impl Fn(&Json) -> bool) -> Json {
        let deadline = Instant::now() + limit;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.lines.recv_timeout(left) {
                Ok(line) => {
                    let record = Json::parse(&line).unwrap_or_else(|e| panic!("{line}: {e}"));
                    self.seen.push(record.clone());
                    if predicate(&record) {
                        return record;
                    }
                }
                Err(RecvTimeoutError::Timeout) => panic!(
                    "no {what} within {limit:?}; the last record {:?}\nstderr:\n{}",
                    self.seen.last().map(Json::compact),
                    self.stderr()
                ),
                Err(RecvTimeoutError::Disconnected) => panic!(
                    "ignite run ended before {what}; the last record {:?}\nstderr:\n{}",
                    self.seen.last().map(Json::compact),
                    self.stderr()
                ),
            }
        }
    }

    /// `stop`, the exit code, every remaining record consumed.
    fn stop(&mut self) -> i32 {
        self.send("stop");
        let status = wait(&mut self.child, LIVE)
            .unwrap_or_else(|| panic!("ignite run did not stop\nstderr:\n{}", self.stderr()));
        while let Ok(line) = self.lines.recv_timeout(REMAINING) {
            self.seen
                .push(Json::parse(&line).unwrap_or_else(|e| panic!("{line}: {e}")));
        }
        status.code().unwrap_or(-1)
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

fn at_ns(record: &Json) -> Option<i64> {
    record.get("at")?.get("ns")?.as_i64()
}

fn machine(record: &Json) -> &str {
    record.get("machine").and_then(Json::as_str).unwrap_or("")
}

fn event_variant(record: &Json) -> Option<&str> {
    if kind(record) != "event" {
        return None;
    }
    record
        .get("value")?
        .get("fields")?
        .get("kind")?
        .get("variant")?
        .as_str()
}

fn event_fields(record: &Json) -> &[Json] {
    record
        .get("value")
        .and_then(|value| value.get("fields"))
        .and_then(|fields| fields.get("kind"))
        .and_then(|kind| kind.get("fields"))
        .and_then(Json::as_array)
        .unwrap_or(&[])
}

/// The status of a `standing` record of [`CONTRACT`].
fn reachable_status(record: &Json) -> Option<&str> {
    if kind(record) != "standing" {
        return None;
    }
    let fields = record.get("value")?.get("fields")?;
    if fields.get("contract")?.as_str()? != CONTRACT {
        return None;
    }
    fields.get("status")?.get("variant")?.as_str()
}

/// The `answered` of the `HealthCheck` a signal record's sample carries,
/// `false` for a sample without a line (the deadline's).
fn answered(record: &Json) -> Option<bool> {
    let line = record
        .get("value")?
        .get("value")?
        .get("fields")?
        .get("line")?;
    match line.get("variant")?.as_str()? {
        "None" => Some(false),
        "Some" => {
            let text = line.get("fields")?.as_array()?.first()?.as_str()?;
            Json::parse(text)
                .ok()?
                .get("fields")?
                .get("answered")?
                .as_bool()
        }
        _ => None,
    }
}

/// How an attempt of one slot ended.
#[derive(Debug, PartialEq)]
enum Slot {
    Signal(bool),
    Failed,
}

const SLOT_NS: i64 = 10_000_000_000;

/// The end of the attempt of the slot at `at`: its `signal` record or its
/// `AttemptFailed` event.
fn slot(run: &mut Run, at: i64) -> Slot {
    let record = run.wait_for(&format!("the attempt of the slot at {at} ns"), LIVE, |r| {
        at_ns(r) == Some(at)
            && (kind(r) == "signal"
                || (event_variant(r) == Some("AttemptFailed")
                    && machine(r) == "TodoService.health.scheduler"))
    });
    if kind(&record) == "signal" {
        Slot::Signal(answered(&record).unwrap_or_else(|| panic!("{}", record.compact())))
    } else {
        Slot::Failed
    }
}

/// Row 16 on `target`: 3 good slots, 4 failed attempts, 31 good slots, 3
/// slots of a stopped server; the standings of the contract, consecutive
/// repeats collapsed, are `Compliant`, `InsufficientEvidence`, `Compliant`,
/// `Violated`.
fn todo_contract_reaches_every_status(target: &str) {
    let scratch = Scratch::new(&format!("contract-{target}"));
    let mut run = Run::start(&scratch, target);
    let ready = run.wait_for("Ready of the server", BUILD, |r| {
        event_variant(r) == Some("Ready") && machine(r) == "TodoService.server.supervisor"
    });
    let server = event_fields(&ready)
        .first()
        .and_then(Json::as_i64)
        .expect("the pid of Ready");
    let mut at = 0;
    let mut next = |run: &mut Run, expected: Slot, phase: &str| {
        if at > 0 {
            run.send("advance 10 seconds");
        }
        let ended = slot(run, at);
        assert_eq!(ended, expected, "{phase}, slot at {at} ns");
        at += SLOT_NS;
    };
    for _ in 0..3 {
        next(&mut run, Slot::Signal(true), "good");
    }

    let bin = scratch.state.join("bin");
    let probe = fs::read_dir(&bin)
        .unwrap()
        .map(|entry| entry.unwrap().path().join("health"))
        .find(|path| path.is_file())
        .unwrap_or_else(|| panic!("no copied probe executable under {}", bin.display()));
    let kept = probe.with_file_name(".health.kept");
    fs::rename(&probe, &kept).unwrap();
    let failing = probe.with_file_name(".health.failing");
    fs::write(&failing, "#!/bin/sh\nexit 1\n").unwrap();
    fs::set_permissions(
        &failing,
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    )
    .unwrap();
    fs::rename(&failing, &probe).unwrap();
    for _ in 0..4 {
        next(&mut run, Slot::Failed, "probe exits 1");
    }
    fs::rename(&kept, &probe).unwrap();
    for _ in 0..31 {
        next(&mut run, Slot::Signal(true), "restored");
    }

    kill(server, nix::sys::signal::Signal::SIGSTOP);
    for _ in 0..3 {
        next(&mut run, Slot::Signal(false), "server stopped");
    }
    kill(server, nix::sys::signal::Signal::SIGCONT);
    let code = run.stop();
    assert_eq!(code, 0, "stderr:\n{}", run.stderr());

    let mut statuses: Vec<&str> = Vec::new();
    for status in run.seen.iter().filter_map(reachable_status) {
        if statuses.last() != Some(&status) {
            statuses.push(status);
        }
    }
    assert_eq!(
        statuses,
        ["Compliant", "InsufficientEvidence", "Compliant", "Violated"],
        "the standings of {CONTRACT}"
    );
}

// ---- rows 17 to 22: the todo under `ignite host` and `ignite apply` -----------

struct Host {
    child: Option<Child>,
    stderr: PathBuf,
}

impl Host {
    /// Start a host on `flow`'s state in its working directory with `env`,
    /// without waiting for it.
    fn spawn(flow: &Flow, label: &str, env: &[(&str, &str)]) -> Host {
        let stderr = flow.at.root.join(format!("{label}.host.stderr"));
        let mut command = Command::new(env!("CARGO_BIN_EXE_ignite"));
        command
            .args(["host", "--state"])
            .arg(&flow.at.state)
            .current_dir(&flow.at.work)
            .env("CARGO_INCREMENTAL", "0")
            .env("CARGO_PROFILE_DEV_DEBUG", "line-tables-only")
            .env_remove(FAULT)
            .stdin(Stdio::piped())
            .stdout(File::create(flow.at.root.join(format!("{label}.host.stdout"))).unwrap())
            .stderr(File::create(&stderr).unwrap());
        for (key, value) in env {
            command.env(key, value);
        }
        Host {
            child: Some(command.spawn().expect("start ignite host")),
            stderr,
        }
    }

    /// Start a host and wait until it answers on its socket.
    fn start(flow: &Flow, label: &str, env: &[(&str, &str)]) -> Host {
        let mut host = Host::spawn(flow, label, env);
        until(
            "host socket",
            LIVE,
            || {
                assert!(
                    host.child.as_mut().unwrap().try_wait().unwrap().is_none(),
                    "host exited: {}",
                    read(&host.stderr)
                );
                flow.status().is_some_and(|status| {
                    status.get("online").and_then(Json::as_bool) == Some(true)
                })
            },
            || read(&host.stderr),
        );
        host
    }

    /// The host ends by itself: its exit status.
    fn ended(&mut self) -> ExitStatus {
        let child = self.child.as_mut().expect("host is running");
        let Some(status) = wait(child, LIVE) else {
            let _ = child.kill();
            let _ = child.wait();
            self.child = None;
            panic!("host did not end: {}", read(&self.stderr));
        };
        self.child = None;
        status
    }

    /// The host crashed itself at the injected fault `point`.
    fn crashed(&mut self, point: &str) {
        let status = self.ended();
        assert_eq!(
            status.signal(),
            Some(9),
            "host must crash with SIGKILL at {point}: {status:?}\n{}",
            read(&self.stderr)
        );
        assert!(
            read(&self.stderr).contains(&format!("injected crash at {point}")),
            "{}",
            read(&self.stderr)
        );
    }

    fn line(&mut self, line: &str) {
        let stdin = self.child.as_mut().unwrap().stdin.as_mut().unwrap();
        writeln!(stdin, "{line}").unwrap();
        stdin.flush().unwrap();
    }

    fn stop(&mut self) {
        self.line("stop");
        let status = self.ended();
        assert!(
            status.success(),
            "host stop failed: {status:?}\n{}",
            read(&self.stderr)
        );
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take()
            && child.try_wait().ok().flatten().is_none()
        {
            if let Some(stdin) = child.stdin.as_mut() {
                let _ = writeln!(stdin, "stop");
                let _ = stdin.flush();
            }
            if wait(&mut child, STOP_ON_DROP).is_none() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

/// The todo copy of one test under a host, on the LLVM target, its state's
/// `build` the one directory every apply test shares (O41).
struct Flow {
    at: Scratch,
    counter: Cell<u64>,
}

impl Flow {
    fn new(label: &str) -> Flow {
        let at = Scratch::new(label);
        common::apply::shared_build(&at.state);
        Flow {
            at,
            counter: Cell::new(0),
        }
    }

    fn state_arg(&self) -> &str {
        self.at.state.to_str().unwrap()
    }

    /// Run the CLI in the working directory: exit code, stdout, stderr.
    fn run(&self, label: &str, args: &[&str]) -> (Option<i32>, String, String) {
        let sequence = self.counter.get();
        self.counter.set(sequence + 1);
        let stdout = self.at.root.join(format!("{sequence}-{label}.stdout"));
        let stderr = self.at.root.join(format!("{sequence}-{label}.stderr"));
        let mut child = Command::new(env!("CARGO_BIN_EXE_ignite"))
            .args(args)
            .current_dir(&self.at.work)
            .env("CARGO_INCREMENTAL", "0")
            .env("CARGO_PROFILE_DEV_DEBUG", "line-tables-only")
            .env_remove(FAULT)
            .stdin(Stdio::null())
            .stdout(File::create(&stdout).unwrap())
            .stderr(File::create(&stderr).unwrap())
            .spawn()
            .expect("start the CLI");
        let limit = if args.first() == Some(&"plan") {
            BUILD
        } else {
            LIVE
        };
        let Some(status) = wait(&mut child, limit) else {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "{label} exceeded {limit:?}\n{}\n{}",
                read(&stdout),
                read(&stderr)
            );
        };
        (status.code(), read(&stdout), read(&stderr))
    }

    fn cli(&self, label: &str, args: &[&str], expected: i32) -> (String, String) {
        let (code, stdout, stderr) = self.run(label, args);
        assert_eq!(code, Some(expected), "{label}\n{stdout}\n{stderr}");
        (stdout, stderr)
    }

    /// `ignite plan --for-apply --target llvm` of the copy: the saved plan.
    fn prepare(&self, label: &str) -> PathBuf {
        let output = self.at.root.join(format!("{label}.plan.json"));
        let mut args = vec![
            "plan".to_string(),
            self.at.source.display().to_string(),
            "--assembly".into(),
            "local".into(),
            "--for-apply".into(),
            "--state".into(),
            self.state_arg().into(),
            "--out".into(),
            output.display().to_string(),
        ];
        args.extend(common::apply::target_args("llvm"));
        let _build = common::apply::build_lock("llvm");
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        self.cli(label, &args, 0);
        output
    }

    /// `ignite apply` of `plan`: exit code, stdout and stderr.
    fn apply(&self, label: &str, plan: &Path) -> (Option<i32>, String, String) {
        self.run(
            label,
            &["apply", plan.to_str().unwrap(), "--state", self.state_arg()],
        )
    }

    /// `ignite inspect --json`, exit 0.
    fn inspect(&self, label: &str) -> Json {
        let (stdout, _) = self.cli(
            label,
            &["inspect", "--state", self.state_arg(), "--json"],
            0,
        );
        Json::parse(&stdout).unwrap_or_else(|reason| panic!("{label}: {reason}\n{stdout}"))
    }

    /// The host's status through the library, for polling.
    fn status(&self) -> Option<Json> {
        ignite::apply::inspect(&self.at.state, None).ok()
    }

    fn journal_path(&self) -> PathBuf {
        self.at.state.join("host-journal.jsonl")
    }

    /// Every host journal record: kind and value.
    fn journal(&self) -> Vec<(String, Json)> {
        read(&self.journal_path())
            .lines()
            .filter_map(|line| Json::parse(line).ok())
            .map(|record| {
                (
                    kind(&record).to_string(),
                    record.get("value").cloned().unwrap_or(Json::Null),
                )
            })
            .collect()
    }

    /// The run records the host journal copied, with their operation.
    fn run_records(&self) -> Vec<(String, Json)> {
        self.journal()
            .into_iter()
            .filter(|(kind, _)| kind == "revisionRecord")
            .filter_map(|(_, value)| {
                let operation = value.get("operationId")?.as_str()?.to_string();
                Some((operation, value.get("record")?.clone()))
            })
            .collect()
    }

    /// Every live `todod` that runs from this state's generations.
    fn servers(&self) -> Vec<i64> {
        let generations = self.at.state.join("revisions");
        let mut found: Vec<i64> = fs::read_dir("/proc")
            .unwrap()
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let pid = entry.file_name().to_string_lossy().parse().ok()?;
                let stat = fs::read_to_string(entry.path().join("stat")).ok()?;
                let state = stat.rsplit_once(") ")?.1.split_whitespace().next()?;
                if matches!(state, "Z" | "X") {
                    return None;
                }
                let executable = fs::read_link(entry.path().join("exe")).ok()?;
                (executable.starts_with(&generations)
                    && executable.file_name().is_some_and(|file| file == "todod"))
                .then_some(pid)
            })
            .collect();
        found.sort();
        found
    }

    /// Every file under the state with its bytes (the `build` link to the
    /// shared build directory is not followed).
    fn files(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        fn walk(path: &Path, found: &mut BTreeMap<PathBuf, Vec<u8>>) {
            for entry in fs::read_dir(path).unwrap() {
                let entry = entry.unwrap();
                let kind = entry.file_type().unwrap();
                if kind.is_dir() {
                    walk(&entry.path(), found);
                } else if kind.is_file() {
                    found.insert(entry.path(), fs::read(entry.path()).unwrap());
                }
            }
        }
        let mut found = BTreeMap::new();
        walk(&self.at.state, &mut found);
        found
    }

    /// `ignite host` that must refuse to start with `expected`: its stderr.
    fn refused(&self, label: &str, expected: i32) -> String {
        self.cli(label, &["host", "--state", self.state_arg()], expected)
            .1
    }

    fn get(&self, path: &str) -> (u16, String) {
        http(self.at.port, "GET", path, None)
    }
}

/// The pid of the owned `TodoService.server` and whether it is ready.
fn server(status: &Json) -> Option<(i64, bool)> {
    let owner = status.get("ownership")?.get(SERVER)?;
    Some((owner.get("pid")?.as_i64()?, owner.get("ready")?.as_bool()?))
}

/// Wait until `revision` is committed, no operation is active and its
/// server is ready: that server's pid.
fn await_committed(flow: &Flow, host: &Host, revision: i64) -> i64 {
    let mut pid = None;
    until(
        "the committed revision ready",
        LIVE,
        || {
            pid = flow.status().and_then(|status| {
                let settled = status.get("committedRevision").and_then(Json::as_i64)
                    == Some(revision)
                    && status.get("activeOperation") == Some(&Json::Null);
                match server(&status) {
                    Some((pid, true)) if settled => Some(pid),
                    _ => None,
                }
            });
            pid.is_some()
        },
        || {
            format!(
                "{}\n{}",
                flow.status().map(|s| s.compact()).unwrap_or_default(),
                read(&host.stderr)
            )
        },
    );
    pid.unwrap()
}

fn plan_json(path: &Path) -> Json {
    Json::parse(&read(path)).unwrap()
}

fn plan_id(path: &Path) -> String {
    plan_json(path)
        .get("id")
        .and_then(Json::as_str)
        .expect("saved plan id")
        .to_string()
}

/// The action kind a saved plan gives `member`.
fn action(plan: &Path, member: &str) -> String {
    let plan = plan_json(plan);
    plan.get("actions")
        .and_then(|actions| actions.get("members"))
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
        .find(|entry| entry.get("id").and_then(Json::as_str) == Some(member))
        .and_then(|entry| entry.get("kind"))
        .and_then(Json::as_str)
        .unwrap_or_else(|| panic!("no action for {member}: {}", plan.compact()))
        .to_string()
}

/// A host in the flow's working directory with revision 1 of the todo
/// applied (exit 0) and the todo `milk` created (201, id 1).
fn with_milk(flow: &Flow) -> Host {
    let host = Host::start(flow, "first", &[]);
    let first = flow.prepare("first");
    let (code, stdout, stderr) = flow.apply("first-apply", &first);
    assert_eq!(code, Some(0), "{stdout}\n{stderr}");
    await_committed(flow, &host, 1);
    let created = http(flow.at.port, "POST", "/todos", Some(r#"{"title":"milk"}"#));
    assert_eq!(
        created,
        (201, r#"{"id":1,"title":"milk","done":false}"#.to_string())
    );
    assert!(
        flow.at.work.join("todo-data/todos/1").is_dir(),
        "the todo's data lives in the host's working directory"
    );
    host
}

/// Row 17: a todo survives a SIGKILL of its server and a rollout.
fn todo_survives_sigkill_and_a_rollout() {
    let flow = Flow::new("rollout");
    let mut host = with_milk(&flow);
    let (old, ready) = server(&flow.inspect("before-kill")).expect("an owned server");
    assert!(ready);
    kill(old, nix::sys::signal::Signal::SIGKILL);
    until(
        &format!("another server pid ready within {RESTART:?}"),
        RESTART,
        || {
            flow.status()
                .and_then(|status| server(&status))
                .is_some_and(|(pid, ready)| pid != old && ready)
        },
        || flow.status().map(|s| s.compact()).unwrap_or_default(),
    );
    let (restarted, _) = server(&flow.inspect("after-kill")).unwrap();
    assert_ne!(restarted, old);
    assert_eq!(flow.get("/todos"), (200, MILK.to_string()));

    flow.at.edit(HEALTH, &HEALTH.replace(r"ok\n", r"ok 2\n"));
    let second = flow.prepare("second");
    assert_eq!(action(&second, SERVER), "replace");
    let (code, stdout, stderr) = flow.apply("second-apply", &second);
    assert_eq!(code, Some(0), "{stdout}\n{stderr}");
    assert_eq!(flow.get("/health"), (200, "ok 2\n".to_string()));
    assert_eq!(flow.get("/todos"), (200, MILK.to_string()));
    host.stop();
}

/// Row 18 (R11, lost acknowledgement): the rollout of row 17 resumed by a
/// host under `after_dispatch`, which dies after dispatching an effect and
/// before recording its outcome.
///
/// A host under `after_dispatch` dies at the first spawn it dispatches, and
/// a host start dispatches the spawn of the committed revision before
/// anything else (17.5), so the rollout cannot be submitted to such a host.
/// The rollout is therefore made durable first: a host under `after_intent`
/// dies right after journaling the acceptance of the rollout, and the host
/// started under `after_dispatch` resumes it and dies after its dispatch.
fn a_host_killed_after_dispatch_never_reports_an_unobserved_start() {
    let flow = Flow::new("lost-ack");
    let mut host = with_milk(&flow);
    host.stop();

    let mut host = Host::start(&flow, "after-intent", &[(FAULT, "after_intent")]);
    await_committed(&flow, &host, 1);
    flow.at.edit(HEALTH, &HEALTH.replace(r"ok\n", r"ok 2\n"));
    let second = flow.prepare("second");
    assert_eq!(action(&second, SERVER), "replace");
    let id = plan_id(&second);
    let (code, _, stderr) = flow.apply("apply-dies-after-intent", &second);
    assert_ne!(code, Some(0), "the host died while applying: {stderr}");
    host.crashed("after_intent");

    let mut host = Host::spawn(&flow, "after-dispatch", &[(FAULT, "after_dispatch")]);
    host.crashed("after_dispatch");
    until(
        "the dispatched children end with their host (parent-death rule)",
        LIVE,
        || flow.servers().is_empty(),
        || format!("{:?}", flow.servers()),
    );

    let mut host = Host::start(&flow, "restarted", &[]);
    let mut outcome = String::new();
    until(
        "the operation completed or recovery_required",
        LIVE,
        || {
            outcome = flow
                .status()
                .and_then(|status| {
                    status
                        .get("operations")?
                        .get(&id)?
                        .get("state")?
                        .as_str()
                        .map(str::to_string)
                })
                .unwrap_or_default();
            outcome == "completed" || outcome == "recovery_required"
        },
        || {
            format!(
                "{}\n{}",
                flow.status().map(|s| s.compact()).unwrap_or_default(),
                read(&host.stderr)
            )
        },
    );
    let status = flow.inspect("restarted");
    let servers = flow.servers();
    assert!(servers.len() <= 1, "more than one todod: {servers:?}");
    if outcome == "completed" {
        let (pid, ready) = server(&status).expect("the completed server");
        assert!(ready, "{}", status.compact());
        let observed = flow.run_records().iter().any(|(_, record)| {
            event_variant(record) == Some("Ready")
                && event_fields(record).first().and_then(Json::as_i64) == Some(pid)
        });
        assert!(observed, "no Ready record of {pid} in the host journal");
    } else {
        assert_eq!(
            status.get("recoveryReason").and_then(Json::as_str),
            Some("outcome_unknown"),
            "{}",
            status.compact()
        );
    }
    // Every host start runs a revision afresh (17.5): a new run journal
    // whose first record is a `segment` at seq 1 and whose generations
    // count from 1 again, so a generation names one instance only within
    // one run of one operation. The rollout's run after the crash must not
    // record a second start of a generation the crashed host dispatched.
    let mut runs: BTreeMap<String, i64> = BTreeMap::new();
    let mut started = BTreeSet::new();
    for (operation, record) in flow.run_records() {
        if kind(&record) == "segment" && record.get("seq").and_then(Json::as_i64) == Some(1) {
            *runs.entry(operation.clone()).or_default() += 1;
        }
        if event_variant(&record) == Some("Started")
            && machine(&record) == format!("{SERVER}.supervisor")
        {
            let generation = event_fields(&record)
                .get(1)
                .and_then(Json::as_i64)
                .expect("the generation of Started");
            let run = runs.get(&operation).copied().unwrap_or_default();
            assert!(
                started.insert((operation.clone(), run, generation)),
                "two Started records of {SERVER} with generation {generation} in run {run} of {operation}"
            );
        }
    }
    assert!(
        started.iter().any(|(operation, _, _)| *operation == id),
        "the rollout's server started: {started:?}"
    );

    if outcome != "completed" {
        host.line("status");
        let (code, stdout, stderr) = flow.apply("retry", &second);
        assert_eq!(code, Some(0), "{stdout}\n{stderr}");
        let status = flow.inspect("retried");
        assert_eq!(
            status
                .get("operations")
                .and_then(|operations| operations.get(&id))
                .and_then(|receipt| receipt.get("state"))
                .and_then(Json::as_str),
            Some("completed"),
            "{}",
            status.compact()
        );
        assert!(flow.servers().len() <= 1, "{:?}", flow.servers());
    }
    assert_eq!(flow.get("/todos"), (200, MILK.to_string()));
    host.stop();
}

/// Row 19 (R11, stale plan): a plan saved before a source change is
/// refused and nothing changes.
fn a_plan_saved_before_a_source_change_is_refused() {
    let flow = Flow::new("stale");
    let mut host = with_milk(&flow);
    let (pid, _) = server(&flow.inspect("before")).expect("an owned server");
    flow.at.edit(HEALTH, &HEALTH.replace(r"ok\n", r"ok 2\n"));
    let saved = flow.prepare("saved");
    flow.at.edit(r#""not stored\n""#, r#""not saved\n""#);
    let transitions = |flow: &Flow| {
        flow.journal()
            .iter()
            .filter(|(kind, _)| kind == "transition")
            .count()
    };
    let before = transitions(&flow);
    let (code, stdout, stderr) = flow.apply("stale-apply", &saved);
    assert_eq!(code, Some(2), "{stdout}\n{stderr}");
    assert!(
        stderr.contains(&flow.at.source.display().to_string()),
        "the reason names the changed source file: {stderr}"
    );
    let (after, ready) = server(&flow.inspect("after")).expect("an owned server");
    assert_eq!((after, ready), (pid, true), "the server is unchanged");
    assert_eq!(
        transitions(&flow),
        before,
        "a transition followed the refusal"
    );
    assert_eq!(flow.get("/todos"), (200, MILK.to_string()));
    host.stop();
}

/// Row 20 (R11): a torn last journal record is dropped and recorded, and
/// the committed revision serves its data.
fn a_torn_last_journal_record_is_dropped_and_recorded() {
    let flow = Flow::new("torn");
    let mut host = with_milk(&flow);
    host.stop();
    let records = read(&flow.journal_path()).lines().count();
    let torn = b"{\"kind\":\"sta";
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(flow.journal_path())
        .unwrap();
    file.write_all(torn).unwrap();
    drop(file);

    let mut host = Host::start(&flow, "repair", &[]);
    await_committed(&flow, &host, 1);
    let digest = ignite::apply::digest::bytes(torn);
    let status = flow.inspect("repaired");
    let repairs = status
        .get("journalRepairs")
        .and_then(Json::as_array)
        .unwrap_or_else(|| panic!("no journalRepairs: {}", status.compact()));
    assert_eq!(repairs.len(), 1, "{}", status.compact());
    assert_eq!(
        repairs[0].get("droppedBytes").and_then(Json::as_i64),
        Some(12)
    );
    assert_eq!(
        repairs[0].get("droppedDigest").and_then(Json::as_str),
        Some(digest.as_str())
    );
    assert_eq!(flow.get("/todos"), (200, MILK.to_string()));
    host.stop();
    assert!(
        read(&host.stderr).contains("ignite host: dropped a torn final journal record of 12 bytes"),
        "{}",
        read(&host.stderr)
    );
    let journal = flow.journal();
    let (kind, value) = &journal[records];
    assert_eq!(
        kind, "journalRepaired",
        "the first new record is the repair"
    );
    assert_eq!(value.get("droppedBytes").and_then(Json::as_i64), Some(12));
    assert_eq!(
        value.get("droppedDigest").and_then(Json::as_str),
        Some(digest.as_str())
    );
}

/// Offline `inspect --json` of a damaged state: exit 0, offline, `journal`
/// as given and the next action of row 21.
fn assert_damaged(flow: &Flow, label: &str, record: i64, reason: &str) {
    let status = flow.inspect(label);
    assert_eq!(
        status.get("online").and_then(Json::as_bool),
        Some(false),
        "{}",
        status.compact()
    );
    assert_eq!(
        status.get("journal").map(Json::compact),
        Some(format!(
            "{{\"intact\":false,\"reason\":\"{reason}\",\"record\":{record}}}"
        )),
        "{}",
        status.compact()
    );
    assert_eq!(
        status.get("nextAction").and_then(Json::as_str),
        Some(DAMAGED),
        "{}",
        status.compact()
    );
}

/// A host that refuses a damaged journal with exit 3 and `message`, starts
/// no process and changes no file of the state.
fn assert_refused(flow: &Flow, label: &str, message: &str) {
    let before = flow.files();
    let stderr = flow.refused(label, 3);
    assert!(stderr.contains(message), "{stderr}");
    assert_eq!(
        flow.servers(),
        Vec::<i64>::new(),
        "a refused host started a server"
    );
    assert_eq!(flow.files(), before, "a refused host changed a file");
}

/// Row 21 (R11): an altered last record refuses the host.
fn an_altered_last_journal_record_refuses_the_host() {
    let flow = Flow::new("altered");
    let mut host = with_milk(&flow);
    host.stop();
    let good = fs::read(flow.journal_path()).unwrap();
    let count = good.iter().filter(|byte| **byte == b'\n').count();
    let start = good[..good.len() - 1]
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |at| at + 1);
    let value = b"\"value\":";
    let inside = start
        + good[start..]
            .windows(value.len())
            .position(|window| window == value)
            .expect("the last record has a value");
    let key = b"\"committedRevision\":";
    let at = inside
        + good[inside..]
            .windows(key.len())
            .position(|window| window == key)
            .expect("the last record is a state record")
        + key.len();
    let mut altered = good.clone();
    altered[at] = if altered[at] == b'7' { b'8' } else { b'7' };
    assert!(Json::parse(std::str::from_utf8(&altered[start..]).unwrap().trim_end()).is_ok());
    fs::write(flow.journal_path(), &altered).unwrap();

    assert_refused(
        &flow,
        "altered",
        &format!("error: host journal record {count}: digest mismatch"),
    );
    assert_damaged(&flow, "altered-offline", count as i64, "digest mismatch");
}

/// Row 22 (R11): the journal cut at the end of the line before its last
/// `state` record refuses the host.
fn a_journal_cut_before_its_last_state_record_refuses_the_host() {
    let flow = Flow::new("cut");
    let mut host = with_milk(&flow);
    host.stop();
    let good = fs::read(flow.journal_path()).unwrap();
    let lines: Vec<&[u8]> = good.split_inclusive(|byte| *byte == b'\n').collect();
    let last_state = lines
        .iter()
        .rposition(|line| {
            Json::parse(std::str::from_utf8(line).unwrap().trim_end())
                .is_ok_and(|record| kind(&record) == "state")
        })
        .expect("a state record");
    let anchor = Json::parse(&read(&flow.at.state.join("status.json")))
        .unwrap()
        .get("journalAnchor")
        .and_then(|anchor| anchor.get("record"))
        .and_then(Json::as_i64)
        .expect("status.json anchors a record");
    assert_eq!(
        anchor,
        last_state as i64 + 1,
        "the anchor is the last state record"
    );
    let cut: usize = lines[..last_state].iter().map(|line| line.len()).sum();
    fs::write(flow.journal_path(), &good[..cut]).unwrap();

    assert_refused(
        &flow,
        "cut",
        &format!("error: host journal ends before record {anchor} that status.json anchors"),
    );
    assert_damaged(&flow, "cut-offline", anchor, "shorter than status.json");
}

mod rust {
    #[test]
    fn todo_contract_reaches_every_status() {
        super::todo_contract_reaches_every_status("rust");
    }
}

mod llvm {
    #[test]
    fn todo_contract_reaches_every_status() {
        super::todo_contract_reaches_every_status("llvm");
    }

    #[test]
    fn todo_survives_sigkill_and_a_rollout() {
        super::todo_survives_sigkill_and_a_rollout();
    }

    #[test]
    fn a_host_killed_after_dispatch_never_reports_an_unobserved_start() {
        super::a_host_killed_after_dispatch_never_reports_an_unobserved_start();
    }

    #[test]
    fn a_plan_saved_before_a_source_change_is_refused() {
        super::a_plan_saved_before_a_source_change_is_refused();
    }

    #[test]
    fn a_torn_last_journal_record_is_dropped_and_recorded() {
        super::a_torn_last_journal_record_is_dropped_and_recorded();
    }

    #[test]
    fn an_altered_last_journal_record_refuses_the_host() {
        super::an_altered_last_journal_record_refuses_the_host();
    }

    #[test]
    fn a_journal_cut_before_its_last_state_record_refuses_the_host() {
        super::a_journal_cut_before_its_last_state_record_refuses_the_host();
    }
}
