//! Exercise frozen LLVM `ignite run` and `ignite replay` without changing benchmark source
//! files (the former `scripts/verify-llvm-readiness.py`): a manual-clock restart with exact
//! backoff, a real-clock restart inside 100 to 500 ms, and a probe timeout with its violated
//! and recovered standings and one breach consequence. Every scenario ends with `stop` and a
//! replay of its journal. `report.json` keeps the Python verifier's fields.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use nix::errno::Errno;
use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;

use crate::cli::ReadinessArgs;
use crate::json::{Json, obj};
use crate::perf::{R, ensure, exit_code, io_err, path_str, resolve, sha256};

pub const MS: i64 = 1_000_000;
pub const SUPERVISOR: &str = "Terminal.server.supervisor";

/// `value["fields"]` of an object, else an empty object.
pub fn fields(value: &Json) -> &Json {
    static EMPTY: Json = Json::Obj(Vec::new());
    if value.is_obj() {
        value.get("fields").unwrap_or(&EMPTY)
    } else {
        &EMPTY
    }
}

/// The event kind of an `event` record, else null.
pub fn event(record: &Json) -> &Json {
    static EMPTY: Json = Json::Obj(Vec::new());
    if record.field("kind").as_str() == Some("event") {
        fields(record.field("value")).get("kind").unwrap_or(&EMPTY)
    } else {
        &EMPTY
    }
}

pub fn event_is(record: &Json, variant: &str) -> bool {
    event(record).field("variant").as_str() == Some(variant)
        && record.field("machine").as_str() == Some(SUPERVISOR)
}

pub fn effect_is(record: &Json, variant: &str) -> bool {
    record.field("kind").as_str() == Some("effect")
        && record.field("value").field("variant").as_str() == Some(variant)
}

pub fn at(record: &Json) -> i64 {
    record.field("at").field("ns").as_i64().unwrap_or(i64::MIN)
}

pub fn standing(record: &Json, contract: &str) -> bool {
    record.field("kind").as_str() == Some("standing")
        && fields(record.field("value")).field("contract").as_str() == Some(contract)
}

pub fn reachable(record: &Json) -> bool {
    standing(record, "Terminal.reachable")
}

/// A probe-attempt signal whose `received` is exactly `value` (Python `is`). In an
/// `ignite-journal/0.2` journal the signal's value is a `std.sli.Sample` (`docs/slice.md`
/// 22.3): a sample without a line is the deadline's, `received` false; else `received` is
/// read from the `ProbeAttempt` its line encodes. The recorded `ignite-journal/0.1` runs
/// carry the `ProbeAttempt` itself.
pub fn received(record: &Json, value: bool) -> bool {
    record.field("kind").as_str() == Some("signal")
        && record.field("value").field("ref").as_str() == Some("Terminal.promptProbe.attempts")
        && sample_received(record.field("value").field("value")) == Some(value)
}

/// Whether the prompt arrived in a signal value: a `std.sli.Sample`, or a 0.1 `ProbeAttempt`.
fn sample_received(sample: &Json) -> Option<bool> {
    if sample.field("type").as_str() != Some("std.sli.Sample") {
        return attempt_received(sample);
    }
    let line = fields(sample).field("line");
    match line.field("variant").as_str()? {
        "None" => Some(false),
        "Some" => line_received(line.field("fields").item(0)),
        _ => None,
    }
}

/// The `received` field of a probe line: the JSON string of a `ProbeAttempt`, or in a 0.1
/// journal the decoded `ProbeAttempt`.
fn line_received(line: &Json) -> Option<bool> {
    match line.as_str() {
        Some(text) => attempt_received(&Json::parse(text).ok()?),
        None => attempt_received(line),
    }
}

fn attempt_received(attempt: &Json) -> Option<bool> {
    match fields(attempt).get("received") {
        Some(Json::Bool(b)) => Some(*b),
        _ => None,
    }
}

fn note_contains(record: &Json, text: &str) -> bool {
    effect_is(record, "Note")
        && record
            .field("value")
            .field("fields")
            .item(0)
            .as_str()
            .is_some_and(|s| s.contains(text))
}

pub fn source_check(repo: &Path, manifest: &Json) -> R<()> {
    for (name, expected) in manifest.as_obj() {
        let file = repo.join(name);
        let same = file.is_file()
            && std::fs::read(&file).ok().map(|b| sha256(&b)).as_deref() == expected.as_str();
        ensure(same, format!("Source manifest mismatch: {name}"))?;
    }
    Ok(())
}

/// The pids of the supervisor's `Started` events, in order.
pub fn pids(seen: &[Json]) -> Vec<i64> {
    seen.iter()
        .filter(|row| event_is(row, "Started"))
        .filter_map(|row| event(row).field("fields").item(0).as_i64())
        .collect()
}

/// What a scenario needs of a controller: its `--json` records, its stdin, the owned
/// processes, the terminal ingress, and the stop with its replay.
pub trait Controller {
    /// The next record matching `predicate`; every record read is kept in [`Controller::seen`].
    fn wait(&mut self, what: &str, predicate: &dyn Fn(&Json) -> bool) -> R<Json>;
    fn send(&mut self, text: &str) -> R<()>;
    fn seen(&self) -> &[Json];
    fn signal(&mut self, pid: i64, signal: Signal) -> R<()>;
    fn echo(&mut self, port: u16) -> R<()>;
    /// `stop`, the exit status 0, no live owned process, and `ignite replay`'s stdout.
    fn stop_and_replay(&mut self) -> R<String>;
    fn journal(&self) -> R<String>;

    fn event(&mut self, variant: &str) -> R<Json> {
        self.wait(&format!("supervisor {variant}"), &|row| {
            event_is(row, variant)
        })
    }

    fn status(&mut self) -> R<Json> {
        self.send("status")?;
        self.wait("status", &|row| {
            row.field("kind").as_str() == Some("status")
        })
    }

    fn pids(&self) -> Vec<i64> {
        pids(self.seen())
    }
}

/// A controller run already recorded: its stdout records, replay stdout and journal, as a
/// verifier work directory keeps them. Commands and signals are accepted and not delivered.
pub struct Recorded {
    pub records: std::collections::VecDeque<Json>,
    pub seen: Vec<Json>,
    pub replay: String,
    pub journal: String,
}

impl Recorded {
    /// The records of `<name>-stdout.jsonl`, `<name>-replay.stdout` and
    /// `states/<name>/journal.jsonl` under `work`.
    pub fn load(work: &Path, name: &str) -> R<Recorded> {
        let read = |path: PathBuf| std::fs::read_to_string(&path).map_err(io_err(path.display()));
        let mut records = std::collections::VecDeque::new();
        for line in read(work.join(format!("{name}-stdout.jsonl")))?.lines() {
            records.push_back(Json::parse(line)?);
        }
        Ok(Recorded {
            records,
            seen: Vec::new(),
            replay: read(work.join(format!("{name}-replay.stdout")))?,
            journal: read(work.join("states").join(name).join("journal.jsonl"))?,
        })
    }
}

impl Controller for Recorded {
    fn wait(&mut self, what: &str, predicate: &dyn Fn(&Json) -> bool) -> R<Json> {
        while let Some(value) = self.records.pop_front() {
            self.seen.push(value.clone());
            if predicate(&value) {
                return Ok(value);
            }
        }
        Err(format!("Controller ended waiting for {what}"))
    }

    fn send(&mut self, _text: &str) -> R<()> {
        Ok(())
    }

    fn seen(&self) -> &[Json] {
        &self.seen
    }

    fn signal(&mut self, _pid: i64, _signal: Signal) -> R<()> {
        Ok(())
    }

    fn echo(&mut self, _port: u16) -> R<()> {
        Ok(())
    }

    fn stop_and_replay(&mut self) -> R<String> {
        Ok(self.replay.clone())
    }

    fn journal(&self) -> R<String> {
        Ok(self.journal.clone())
    }
}

impl Controller for Run {
    fn wait(&mut self, what: &str, predicate: &dyn Fn(&Json) -> bool) -> R<Json> {
        self.wait_for(what, predicate)
    }

    fn send(&mut self, text: &str) -> R<()> {
        self.send_line(text)
    }

    fn seen(&self) -> &[Json] {
        &self.seen
    }

    fn signal(&mut self, pid: i64, signal: Signal) -> R<()> {
        self.signal_pid(pid, signal)
    }

    fn echo(&mut self, port: u16) -> R<()> {
        echo(port)
    }

    fn stop_and_replay(&mut self) -> R<String> {
        self.stop_replay()
    }

    fn journal(&self) -> R<String> {
        let path = self.state.join("journal.jsonl");
        std::fs::read_to_string(&path).map_err(io_err(path.display()))
    }
}

enum Message {
    Record(Json),
    Failure(String),
    End,
}

fn wait_child(child: &mut Child, timeout: Duration) -> R<i64> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().map_err(io_err("wait"))? {
            return Ok(exit_code(status));
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "Timed out after {} s waiting for a process",
                timeout.as_secs_f64()
            ));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn join_within(handle: Option<JoinHandle<()>>, timeout: Duration) -> bool {
    let Some(handle) = handle else { return true };
    let deadline = Instant::now() + timeout;
    while !handle.is_finished() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    if handle.is_finished() {
        let _ = handle.join();
        true
    } else {
        false
    }
}

fn own_executable(state: &Path, pid: i64) -> bool {
    std::fs::canonicalize(format!("/proc/{pid}/exe"))
        .is_ok_and(|exe| exe.starts_with(state.join("bin")))
}

/// One `ignite run --json` controller of the design file's `local` assembly.
pub struct Run {
    work: PathBuf,
    repo: PathBuf,
    cli: PathBuf,
    timeout: Duration,
    name: String,
    pub state: PathBuf,
    pub seen: Vec<Json>,
    lines: Arc<Mutex<Vec<String>>>,
    errors: Arc<Mutex<Vec<String>>>,
    messages: Receiver<Message>,
    stopped: Vec<i64>,
    command: Vec<String>,
    child: Child,
    stdin: Option<ChildStdin>,
    out_thread: Option<JoinHandle<()>>,
    err_thread: Option<JoinHandle<()>>,
}

/// The `ignite run` command line of one scenario.
pub fn run_command(
    args: &ReadinessArgs,
    state: &Path,
    clock: &str,
    epoch: Option<&str>,
) -> Vec<String> {
    let mut command: Vec<String> = [
        path_str(&args.cli),
        "run".into(),
        path_str(&args.repo.join("examples/terminal/terminal.design.ignite")),
        "--assembly".into(),
        "local".into(),
        "--state".into(),
        path_str(state),
        "--build".into(),
        path_str(&args.work.join("build")),
        "--target".into(),
        "llvm".into(),
        "--runtime".into(),
        path_str(&args.runtime),
        "--json".into(),
        "--settle".into(),
        "1 s".into(),
        "--port".into(),
        "Terminal.ingress=0".into(),
        "--clock".into(),
        clock.into(),
    ]
    .into();
    if let Some(epoch) = epoch {
        command.push("--epoch".into());
        command.push(epoch.into());
    }
    command
}

impl Run {
    pub fn start(args: &ReadinessArgs, name: &str, clock: &str, epoch: Option<&str>) -> R<Run> {
        let state = args.work.join("states").join(name);
        ensure(
            !state.exists(),
            format!("Refusing to reuse existing state: {}", state.display()),
        )?;
        let command = run_command(args, &state, clock, epoch);
        let mut child = Command::new(&command[0])
            .args(&command[1..])
            .current_dir(&args.repo)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(io_err(&command[0]))?;
        let (send, messages) = mpsc::channel();
        let lines = Arc::new(Mutex::new(Vec::new()));
        let errors = Arc::new(Mutex::new(Vec::new()));
        let stdout = child.stdout.take().expect("piped stdout");
        let out_lines = Arc::clone(&lines);
        let out_send = send.clone();
        let out_thread = std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) => break,
                    Ok(_) => {
                        out_lines.lock().expect("lines").push(line.clone());
                        match Json::parse(line.trim_end_matches('\n')) {
                            Ok(value) => {
                                let _ = out_send.send(Message::Record(value));
                            }
                            Err(error) => {
                                let _ = out_send.send(Message::Failure(format!(
                                    "Invalid --json record: {line:?}: {error}"
                                )));
                                break;
                            }
                        }
                    }
                    Err(error) => {
                        let _ = out_send.send(Message::Failure(error.to_string()));
                        break;
                    }
                }
            }
            let _ = out_send.send(Message::End);
        });
        let stderr = child.stderr.take().expect("piped stderr");
        let err_lines = Arc::clone(&errors);
        let err_thread = std::thread::spawn(move || {
            let mut reader = BufReader::new(stderr);
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) => break,
                    Ok(_) => err_lines.lock().expect("errors").push(line),
                    Err(error) => {
                        let _ = send.send(Message::Failure(error.to_string()));
                        break;
                    }
                }
            }
        });
        Ok(Run {
            work: args.work.clone(),
            repo: args.repo.clone(),
            cli: args.cli.clone(),
            timeout: Duration::from_secs_f64(args.timeout),
            name: name.into(),
            state,
            seen: Vec::new(),
            lines,
            errors,
            messages,
            stopped: Vec::new(),
            command,
            stdin: child.stdin.take(),
            child,
            out_thread: Some(out_thread),
            err_thread: Some(err_thread),
        })
    }

    fn stderr_text(&self) -> String {
        self.errors.lock().expect("errors").concat()
    }

    fn send_line(&mut self, text: &str) -> R<()> {
        let running = self.child.try_wait().map_err(io_err("wait"))?.is_none();
        ensure(running, format!("Controller exited before command: {text}"))?;
        let stdin = self.stdin.as_mut().ok_or("Controller stdin is closed")?;
        stdin
            .write_all(format!("{text}\n").as_bytes())
            .and_then(|_| stdin.flush())
            .map_err(io_err("controller stdin"))
    }

    fn wait_for(&mut self, what: &str, predicate: &dyn Fn(&Json) -> bool) -> R<Json> {
        let deadline = Instant::now() + self.timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self
                .messages
                .recv_timeout(left.max(Duration::from_millis(1)))
            {
                Ok(Message::Record(value)) => {
                    self.seen.push(value.clone());
                    if predicate(&value) {
                        return Ok(value);
                    }
                }
                Ok(Message::Failure(error)) => return Err(error),
                Ok(Message::End) | Err(RecvTimeoutError::Disconnected) => {
                    return Err(format!(
                        "Controller ended waiting for {what}\n{}",
                        self.stderr_text()
                    ));
                }
                Err(RecvTimeoutError::Timeout) => {}
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "Timed out waiting for {what}\n{}",
                    self.stderr_text()
                ));
            }
        }
    }

    fn signal_pid(&mut self, pid: i64, signal: Signal) -> R<()> {
        ensure(
            own_executable(&self.state, pid),
            format!("Refusing to signal a non-owned process: {pid}"),
        )?;
        kill(Pid::from_raw(pid as i32), signal).map_err(|e| format!("kill {pid}: {e}"))?;
        if signal == Signal::SIGSTOP {
            self.stopped.push(pid);
        }
        if signal == Signal::SIGCONT {
            self.stopped.retain(|p| *p != pid);
        }
        Ok(())
    }

    fn owned_pids(&self) -> Vec<i64> {
        let mut result = pids(&self.seen);
        if let Ok(entries) = std::fs::read_dir("/proc") {
            for entry in entries.flatten() {
                if let Some(pid) = entry
                    .file_name()
                    .to_str()
                    .and_then(|n| n.parse::<i64>().ok())
                    && own_executable(&self.state, pid)
                    && !result.contains(&pid)
                {
                    result.push(pid);
                }
            }
        }
        result
    }

    fn no_live_processes(&self) -> R<()> {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let mut live = Vec::new();
            for pid in self.owned_pids() {
                if !own_executable(&self.state, pid) {
                    continue;
                }
                let Ok(status) = std::fs::read_to_string(format!("/proc/{pid}/status")) else {
                    continue;
                };
                let state = status
                    .lines()
                    .find(|l| l.starts_with("State:"))
                    .unwrap_or("");
                if !state.contains('Z') {
                    live.push(pid);
                }
            }
            if live.is_empty() {
                return Ok(());
            }
            ensure(
                Instant::now() < deadline,
                format!("Processes remain after stop: {live:?}"),
            )?;
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn stop_replay(&mut self) -> R<String> {
        self.send_line("stop")?;
        let code = wait_child(&mut self.child, self.timeout)?;
        let out_done = join_within(self.out_thread.take(), Duration::from_secs(2));
        let err_done = join_within(self.err_thread.take(), Duration::from_secs(2));
        ensure(
            out_done && err_done,
            "Controller output readers did not finish",
        )?;
        while let Ok(pending) = self.messages.try_recv() {
            if let Message::Failure(error) = pending {
                return Err(error);
            }
        }
        ensure(
            code == 0,
            format!("stop exited {code}\n{}", self.stderr_text()),
        )?;
        self.no_live_processes()?;
        let command = [
            path_str(&self.cli),
            "replay".into(),
            path_str(&self.repo.join("examples/terminal/terminal.design.ignite")),
            "--assembly".into(),
            "local".into(),
            "--state".into(),
            path_str(&self.state),
        ];
        let (code, stdout, stderr) = capture(&command, &self.repo, self.timeout)?;
        write(
            &self.work.join(format!("{}-replay.stdout", self.name)),
            &stdout,
        )?;
        write(
            &self.work.join(format!("{}-replay.stderr", self.name)),
            &stderr,
        )?;
        ensure(code == 0, format!("replay exited {code}: {stderr}"))?;
        Ok(stdout)
    }

    pub fn cleanup(&mut self) -> R<()> {
        for pid in std::mem::take(&mut self.stopped) {
            if own_executable(&self.state, pid) {
                match kill(Pid::from_raw(pid as i32), Signal::SIGCONT) {
                    Ok(()) | Err(Errno::ESRCH) => {}
                    Err(error) => return Err(format!("kill {pid}: {error}")),
                }
            }
        }
        if self.child.try_wait().map_err(io_err("wait"))?.is_none() {
            let stopped = self
                .send_line("stop")
                .and_then(|_| wait_child(&mut self.child, Duration::from_secs(3)));
            if stopped.is_err() {
                for pid in self.owned_pids() {
                    if own_executable(&self.state, pid) {
                        let _ = kill(Pid::from_raw(pid as i32), Signal::SIGKILL);
                    }
                }
                let _ = self.child.kill();
                wait_child(&mut self.child, Duration::from_secs(3))?;
            }
        }
        join_within(self.out_thread.take(), Duration::from_secs(2));
        join_within(self.err_thread.take(), Duration::from_secs(2));
        let lines = self.lines.lock().expect("lines").concat();
        write(
            &self.work.join(format!("{}-stdout.jsonl", self.name)),
            &lines,
        )?;
        write(
            &self.work.join(format!("{}-stderr.log", self.name)),
            &self.stderr_text(),
        )?;
        let command = Json::from(self.command.clone()).dumps_indent(false, 2) + "\n";
        write(
            &self.work.join(format!("{}-command.json", self.name)),
            &command,
        )
    }
}

fn write(path: &Path, text: &str) -> R<()> {
    std::fs::write(path, text).map_err(io_err(path.display()))
}

/// `subprocess.run(command, cwd=cwd, capture_output=True, text=True, timeout=timeout)`.
fn capture(command: &[String], cwd: &Path, timeout: Duration) -> R<(i64, String, String)> {
    let mut child = Command::new(&command[0])
        .args(&command[1..])
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(io_err(&command[0]))?;
    let reader = |mut pipe: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = pipe.read_to_end(&mut bytes);
            String::from_utf8_lossy(&bytes).into_owned()
        })
    };
    let out = reader(Box::new(child.stdout.take().expect("piped stdout")));
    let err = reader(Box::new(child.stderr.take().expect("piped stderr")));
    let code = match wait_child(&mut child, timeout) {
        Ok(code) => code,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    let stdout = out.join().map_err(|_| "stdout reader panicked")?;
    let stderr = err.join().map_err(|_| "stderr reader panicked")?;
    Ok((code, stdout, stderr))
}

fn with_run<T>(
    args: &ReadinessArgs,
    name: &str,
    clock: &str,
    epoch: Option<&str>,
    body: impl FnOnce(&mut dyn Controller) -> R<T>,
) -> R<T> {
    let mut run = Run::start(args, name, clock, epoch)?;
    let result = body(&mut run);
    let cleaned = run.cleanup();
    cleaned.and(result)
}

fn read_until(connection: &mut TcpStream, done: impl Fn(&[u8]) -> bool, what: &str) -> R<()> {
    let mut seen = Vec::new();
    let mut chunk = [0u8; 1024];
    while !done(&seen) {
        let n = connection.read(&mut chunk).map_err(io_err("terminal"))?;
        ensure(n > 0, format!("EOF before {what}"))?;
        seen.extend_from_slice(&chunk[..n]);
    }
    Ok(())
}

fn echo(port: u16) -> R<()> {
    let address = SocketAddr::from(([127, 0, 0, 1], port));
    let mut connection =
        TcpStream::connect_timeout(&address, Duration::from_secs(5)).map_err(io_err("connect"))?;
    connection
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(io_err("socket"))?;
    connection
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(io_err("socket"))?;
    read_until(
        &mut connection,
        |s| s.ends_with(b"terminal> "),
        "terminal prompt",
    )?;
    connection
        .write_all(b"echo hello\r\n")
        .map_err(io_err("send"))?;
    read_until(
        &mut connection,
        |s| s.windows(5).any(|w| w == b"hello"),
        "echo reply",
    )?;
    connection.write_all(b"quit\r\n").map_err(io_err("send"))
}

fn ended(row: &Json) -> bool {
    event_is(row, "Exited") || event_is(row, "Signalled")
}

fn first_pid(run: &dyn Controller) -> R<i64> {
    run.pids()
        .first()
        .copied()
        .ok_or_else(|| "No Started pid".to_string())
}

/// Manual clock: echo through the isolated ingress, kill the server, restart after exactly
/// 100 ms of advanced time (not at 99 ms), then a second kill backs off 200 ms.
pub fn manual_scenario(run: &mut dyn Controller) -> R<Json> {
    run.event("Ready")?;
    let status = run.status()?;
    let port = status.field("ports").field("Terminal.ingress").as_i64();
    let port = port.filter(|p| (1..65536).contains(p) && *p != 2323);
    let port = port.ok_or("Missing isolated ingress port")?;
    run.echo(port as u16)?;
    let pid = first_pid(run)?;
    run.signal(pid, Signal::SIGKILL)?;
    let exit = run.wait("supervisor exit", &ended)?;
    ensure(
        event(&exit).field("fields").item(0).as_i64() == Some(pid),
        "Exited pid differs",
    )?;
    run.wait("restart attempt 1", &|row| {
        note_contains(row, "restart attempt 1 delay 100 ms")
    })?;
    let timer = run.wait("restart timer", &|row| {
        effect_is(row, "SetTimer") && row.field("machine").as_str() == Some(SUPERVISOR)
    })?;
    let due = timer.field("value").field("fields").item(1).field("ns");
    ensure(
        due.as_i64() == Some(at(&exit) + 100 * MS),
        "Incorrect backoff timer",
    )?;
    run.send("advance 99 ms")?;
    let before = run.seen().len();
    run.status()?;
    ensure(
        !run.seen()[before..]
            .iter()
            .any(|row| event_is(row, "Started")),
        "Restart before 100 ms",
    )?;
    run.send("advance 1 ms")?;
    let started = run.event("Started")?;
    ensure(
        at(&started) - at(&exit) == 100 * MS,
        "Manual restart differs from exact 100 ms",
    )?;
    run.event("Ready")?;
    let last = *run.pids().last().ok_or("No Started pid")?;
    run.signal(last, Signal::SIGKILL)?;
    run.wait("restart attempt 2", &|row| {
        note_contains(row, "restart attempt 2 delay 200 ms")
    })?;
    run.stop_and_replay()?;
    Ok(obj(vec![
        ("manual_restart_ns", Json::Int(at(&started) - at(&exit))),
        ("second_backoff_ms", Json::Int(200)),
        ("echo", true.into()),
        ("stopped", true.into()),
        ("replay", true.into()),
    ]))
}

/// Real clock: a killed server restarts within 100 to 500 ms.
pub fn real_scenario(run: &mut dyn Controller) -> R<Json> {
    run.event("Ready")?;
    let pid = first_pid(run)?;
    run.signal(pid, Signal::SIGKILL)?;
    let exit = run.wait("supervisor exit", &ended)?;
    let started = run.event("Started")?;
    let delay = at(&started) - at(&exit);
    ensure(
        (100 * MS..=500 * MS).contains(&delay),
        format!("Real restart {delay} ns outside 100–500 ms"),
    )?;
    run.event("Ready")?;
    run.stop_and_replay()?;
    Ok(obj(vec![
        ("real_restart_ns", Json::Int(delay)),
        ("stopped", true.into()),
        ("replay", true.into()),
    ]))
}

/// `fields(value)[key]` as an integer.
fn count(value: &Json, key: &str) -> Option<i64> {
    fields(value).field(key).as_i64()
}

fn status_variant(value: &Json) -> Option<&str> {
    fields(value).field("status").field("variant").as_str()
}

/// Evidence of the timed-out attempt: a `probe timeout` note or a bad `AttemptDone`.
pub fn timeout_seen(seen: &[Json]) -> bool {
    seen.iter().any(|row| {
        let note = effect_is(row, "Note")
            && row.field("value").field("fields").item(0).as_str() == Some("probe timeout");
        let kind = event(row);
        let done = kind.field("variant").as_str() == Some("AttemptDone")
            && line_received(kind.field("fields").item(1)) == Some(false);
        note || done
    })
}

/// Manual clock from 22:59:30 UTC: a stopped server fails the 10 s probe and violates
/// `Terminal.reachable` (status agrees with the journal); resumed, the next probe is good while
/// the rolling window keeps the violation; the calendar period closing at 30 s is violated and
/// fires its consequence exactly once.
pub fn probe_scenario(run: &mut dyn Controller) -> R<Json> {
    run.event("Ready")?;
    let first = run.wait("first reachable standing", &reachable)?;
    ensure(
        count(first.field("value"), "good") == Some(1)
            && count(first.field("value"), "executed") == Some(1),
        "Initial attempt is not good",
    )?;
    let pid = first_pid(run)?;
    run.signal(pid, Signal::SIGSTOP)?;
    run.send("advance 10 seconds")?;
    let attempt = run.wait("probe attempt", &|row| effect_is(row, "Attempt"))?;
    ensure(
        at(&attempt) == 10_000 * MS,
        "Probe not dispatched at 10 seconds",
    )?;
    run.send("advance 2 seconds")?;
    let bad = run.wait("bad attempt signal", &|row| received(row, false))?;
    ensure(
        [10_000 * MS, 12_000 * MS].contains(&at(&bad)),
        "Unexpected timeout observation instant",
    )?;
    let violated = run.wait("violated reachable standing", &reachable)?;
    let value = violated.field("value");
    ensure(
        status_variant(value) == Some("Violated")
            && count(value, "good") == Some(1)
            && count(value, "executed") == Some(2),
        "Incorrect violated standing",
    )?;
    ensure(
        timeout_seen(run.seen()),
        "Missing timeout/bad AttemptDone evidence",
    )?;
    let status = run.status()?;
    let current = status
        .field("standings")
        .as_arr()
        .iter()
        .find(|s| fields(s).field("contract").as_str() == Some("Terminal.reachable"))
        .ok_or("Status lacks the reachable standing")?;
    ensure(current == value, "Status and journal standing differ")?;
    run.signal(pid, Signal::SIGCONT)?;
    run.send("advance 10 seconds")?;
    let good = run.wait("recovered probe signal", &|row| {
        received(row, true) && at(row) == 20_000 * MS
    })?;
    let still = run.wait("retained rolling violation", &|row| {
        reachable(row) && at(row) == 20_000 * MS
    })?;
    ensure(
        status_variant(still.field("value")) == Some("Violated")
            && count(still.field("value"), "executed") == Some(3),
        "Rolling window forgot failed attempt",
    )?;
    run.send("advance 10 seconds")?;
    let closed = run.wait("closed calendar period", &|row| {
        standing(row, "Terminal.customerAccess") && at(row) == 30_000 * MS
    })?;
    ensure(
        status_variant(closed.field("value")) == Some("Violated"),
        "Closed period is not violated",
    )?;
    let consequence = run.wait("breach consequence", &|row| {
        row.field("kind").as_str() == Some("consequence")
    })?;
    let fired = fields(consequence.field("value"));
    ensure(
        fired.field("consequence").as_str() == Some("access_commitment_breached")
            && fired.field("standing_seq") == closed.field("seq"),
        "Consequence differs from closed standing",
    )?;
    let replay = run.stop_and_replay()?;
    let mut consequences = 0;
    for line in run.journal()?.lines() {
        let row = Json::parse(line).map_err(|e| format!("journal: {e}"))?;
        consequences += usize::from(row.field("kind").as_str() == Some("consequence"));
    }
    ensure(consequences == 1, "Consequence duplicated")?;
    ensure(
        replay.contains("Terminal.reachable"),
        "Replay lacks reachable standing",
    )?;
    Ok(obj(vec![
        ("timeout_at_ns", Json::Int(at(&bad))),
        ("violated_good", Json::Int(1)),
        ("violated_executed", Json::Int(2)),
        ("recovered_signal_at_ns", Json::Int(at(&good))),
        ("rolling_violation_retained", true.into()),
        ("consequence_once", true.into()),
        ("stopped", true.into()),
        ("replay", true.into()),
    ]))
}

/// The scenarios in report order: name, state directory, clock, epoch and body.
pub type Scenario = fn(&mut dyn Controller) -> R<Json>;

pub const SCENARIOS: [(&str, &str, &str, Option<&str>, Scenario); 3] = [
    ("manual", "manual-restart", "manual", None, manual_scenario),
    ("real", "real-restart", "real", None, real_scenario),
    (
        "probe",
        "probe-timeout",
        "manual",
        Some("2026-10-31T22:59:30Z"),
        probe_scenario,
    ),
];

fn file_sha(path: &Path) -> R<String> {
    Ok(sha256(
        &std::fs::read(path).map_err(io_err(path.display()))?,
    ))
}

/// Each scenario in order, its result into `report` and stdout, then the source check again.
fn run_scenarios(args: &ReadinessArgs, manifest: &Json, report: &mut Json) -> R<()> {
    for (name, state, clock, epoch, scenario) in SCENARIOS {
        let result = with_run(args, state, clock, epoch, scenario)?;
        let mut all = report.field("scenarios").clone();
        all.set(name, result.clone());
        report.set("scenarios", all);
        println!("{}", obj(vec![(name, result)]).dumps(false));
    }
    source_check(&args.repo, manifest)?;
    report.set("accepted", true.into());
    Ok(())
}

/// Run the three scenarios and write `report.json` whatever their outcome.
pub fn verify(args: &ReadinessArgs) -> R<()> {
    let mut args = args.clone();
    for path in [
        &mut args.cli,
        &mut args.repo,
        &mut args.runtime,
        &mut args.work,
        &mut args.source_manifest,
    ] {
        *path = resolve(path);
    }
    ensure(
        !args.work.starts_with(&args.repo),
        "Verifier work must be outside frozen source",
    )?;
    let report_path = args.work.join("report.json");
    ensure(
        !report_path.exists(),
        "Refusing to overwrite an existing verifier report",
    )?;
    std::fs::create_dir_all(&args.work).map_err(io_err(args.work.display()))?;
    let manifest = crate::perf::read_json(&args.source_manifest)?;
    source_check(&args.repo, &manifest)?;
    let mut report = obj(vec![
        ("candidate", args.candidate.as_str().into()),
        ("target", "llvm".into()),
        ("cli", path_str(&args.cli).into()),
        ("cli_sha256", file_sha(&args.cli)?.into()),
        ("runtime", path_str(&args.runtime).into()),
        ("runtime_sha256", file_sha(&args.runtime)?.into()),
        (
            "source_manifest_sha256",
            file_sha(&args.source_manifest)?.into(),
        ),
        ("scenarios", obj::<&str>(vec![])),
        ("accepted", false.into()),
    ]);
    let outcome = run_scenarios(&args, &manifest, &mut report);
    if let Err(error) = &outcome {
        report.set("error", error.as_str().into());
    }
    write(&report_path, &(report.dumps_indent(false, 2) + "\n"))?;
    outcome
}

pub fn exit_status(result: R<()>) -> i32 {
    match result {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}
