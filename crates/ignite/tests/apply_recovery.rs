//! Host durability and crash recovery (`docs/slice.md` 17.5, 17.7 rows 10 to
//! 14) through the real CLI, host socket and native program. A debug build
//! of the host crashes itself (SIGKILL) at the point named by
//! `IGNITE_DEBUG_HOST_FAULT`, or fails a named journal sync; the next host
//! on the same state must end with exactly one server generation running,
//! or report `outcome_unknown` for what it cannot prove.

use std::cell::Cell;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ignite_eval::Json;

use crate::common::apply::{BUILD as BUILD_TIMEOUT, LIVE as LIVE_TIMEOUT};

const FAULT: &str = "IGNITE_DEBUG_HOST_FAULT";

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

fn until(what: &str, mut done: impl FnMut() -> bool, context: impl Fn() -> String) {
    let deadline = Instant::now() + LIVE_TIMEOUT;
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

/// A listener on `port` once the port is free, within [`LIVE_TIMEOUT`]: a
/// server that just ended may hold it until the kernel closed its socket.
fn bind_when_free(port: u16) -> TcpListener {
    let deadline = Instant::now() + LIVE_TIMEOUT;
    loop {
        match TcpListener::bind(("127.0.0.1", port)) {
            Ok(listener) => return listener,
            Err(error)
                if error.kind() == std::io::ErrorKind::AddrInUse && Instant::now() < deadline =>
            {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(error) => panic!("a foreign listener on {port}: {error}"),
        }
    }
}

struct Host {
    child: Option<Child>,
    stderr: PathBuf,
}

impl Host {
    /// Start a host on `flow`'s state with `env`, without waiting for it.
    fn spawn(flow: &Flow, label: &str, env: &[(&str, &str)]) -> Host {
        let stderr = flow.root.join(format!("{label}.stderr"));
        let mut command = Command::new(env!("CARGO_BIN_EXE_ignite"));
        command
            .args(["host", "--state"])
            .arg(&flow.state)
            .current_dir(repo())
            .env("CARGO_INCREMENTAL", "0")
            .env("CARGO_PROFILE_DEV_DEBUG", "line-tables-only")
            .env_remove(FAULT)
            .stdin(Stdio::piped())
            .stdout(File::create(flow.root.join(format!("{label}.stdout"))).unwrap())
            .stderr(File::create(&stderr).unwrap());
        for (key, value) in env {
            command.env(key, value);
        }
        Host {
            child: Some(command.spawn().expect("start foreground host")),
            stderr,
        }
    }

    /// Start a host and wait until it answers on its socket.
    fn start(flow: &Flow, label: &str, env: &[(&str, &str)]) -> Host {
        let mut host = Host::spawn(flow, label, env);
        until(
            "host socket",
            || {
                assert!(
                    host.child.as_mut().unwrap().try_wait().unwrap().is_none(),
                    "host exited: {}",
                    read(&host.stderr)
                );
                ignite::apply::inspect(&flow.state, None)
                    .is_ok_and(|status| status.get("online").and_then(Json::as_bool) == Some(true))
            },
            || read(&host.stderr),
        );
        host
    }

    /// The host ends by itself: its exit status.
    fn ended(&mut self) -> ExitStatus {
        let child = self.child.as_mut().expect("host is running");
        let status = wait(child, LIVE_TIMEOUT);
        let Some(status) = status else {
            let _ = child.kill();
            let _ = child.wait();
            self.child = None;
            panic!("host did not end: {}", read(&self.stderr));
        };
        self.child = None;
        status
    }

    /// The host crashed itself at the injected fault point.
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
            if wait(&mut child, Duration::from_secs(10)).is_none() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

/// A child of the test that is killed when the test ends.
struct Owned(Child);

impl Drop for Owned {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

struct Flow {
    root: PathBuf,
    state: PathBuf,
    source: PathBuf,
    port: u16,
    target: &'static str,
    counter: Cell<u64>,
}

impl Flow {
    /// [`Flow::for_target`] on the Rust target.
    fn new(label: &str) -> Flow {
        Flow::for_target(label, "rust")
    }

    /// A fresh copy of the terminal design on a free port, built for
    /// `target`, and a fresh state whose `build` is the one directory every
    /// apply test shares (O41).
    fn for_target(label: &str, target: &'static str) -> Flow {
        let port = free_port();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "recovery-{label}-{target}-{}-{stamp}",
            std::process::id()
        ));
        let fixture = root.join("terminal");
        fs::create_dir_all(&fixture).unwrap();
        for entry in fs::read_dir(repo().join("examples/terminal")).unwrap() {
            let path = entry.unwrap().path();
            if path
                .extension()
                .is_some_and(|extension| extension == "ignite")
            {
                fs::copy(&path, fixture.join(path.file_name().unwrap())).unwrap();
            }
        }
        let source = fixture.join("terminal.design.ignite");
        let original = fs::read_to_string(&source).unwrap();
        assert!(original.contains("port = 2323"), "terminal fixture port");
        fs::write(
            &source,
            original.replace("port = 2323", &format!("port = {port}")),
        )
        .unwrap();
        let state = root.join("state");
        fs::create_dir_all(&state).unwrap();
        crate::common::apply::shared_build(&state);
        Flow {
            root,
            state,
            source,
            port,
            target,
            counter: Cell::new(0),
        }
    }

    /// Run the CLI with `args`: its exit code and stdout.
    fn run(&self, label: &str, args: &[&str]) -> (Option<i32>, String, String) {
        let sequence = self.counter.get();
        self.counter.set(sequence + 1);
        let stdout = self.root.join(format!("{sequence}-{label}.stdout"));
        let stderr = self.root.join(format!("{sequence}-{label}.stderr"));
        let mut child = Command::new(env!("CARGO_BIN_EXE_ignite"))
            .args(args)
            .current_dir(repo())
            .env("CARGO_INCREMENTAL", "0")
            .env("CARGO_PROFILE_DEV_DEBUG", "line-tables-only")
            .stdin(Stdio::null())
            .stdout(File::create(&stdout).unwrap())
            .stderr(File::create(&stderr).unwrap())
            .spawn()
            .expect("start CLI client");
        let limit = if args.first() == Some(&"plan") {
            BUILD_TIMEOUT
        } else {
            LIVE_TIMEOUT
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

    fn cli(&self, label: &str, args: &[&str], expected: i32) -> String {
        let (code, stdout, stderr) = self.run(label, args);
        assert_eq!(code, Some(expected), "{label}\n{stdout}\n{stderr}");
        stdout
    }

    fn prepare(&self, label: &str) -> PathBuf {
        self.prepare_with(label, &[])
    }

    /// [`Flow::prepare`] with `extra` arguments to `plan`.
    fn prepare_with(&self, label: &str, extra: &[&str]) -> PathBuf {
        let output = self.root.join(format!("{label}.json"));
        let selected = crate::common::apply::target_args(self.target);
        let mut args = vec![
            "plan",
            self.source.to_str().unwrap(),
            "--assembly",
            "local",
            "--for-apply",
            "--state",
            self.state.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ];
        args.extend(selected.iter().map(String::as_str));
        args.extend(extra);
        let _build = crate::common::apply::build_lock(self.target);
        self.cli(label, &args, 0);
        output
    }

    fn apply_args<'a>(&'a self, plan: &'a Path) -> [&'a str; 4] {
        [
            "apply",
            plan.to_str().unwrap(),
            "--state",
            self.state.to_str().unwrap(),
        ]
    }

    fn apply(&self, label: &str, plan: &Path) -> Json {
        let output = self.cli(label, &self.apply_args(plan), 0);
        Json::parse(&output).unwrap_or_else(|reason| panic!("{label}: {reason}\n{output}"))
    }

    /// An apply the host does not complete: its exit code.
    fn apply_fails(&self, label: &str, plan: &Path) -> (Option<i32>, String) {
        let (code, _, stderr) = self.run(label, &self.apply_args(plan));
        assert_ne!(code, Some(0), "{label} must not complete: {stderr}");
        (code, stderr)
    }

    fn inspect(&self) -> Json {
        ignite::apply::inspect(&self.state, None).expect("inspect")
    }

    /// The journal's records of `kind`.
    fn journal(&self, kind: &str) -> Vec<Json> {
        read(&self.state.join("host-journal.jsonl"))
            .lines()
            .filter_map(|line| Json::parse(line).ok())
            .filter(|record| record.get("kind").and_then(Json::as_str) == Some(kind))
            .filter_map(|record| record.get("value").cloned())
            .collect()
    }

    /// The `revisionRecord`s of `operation` whose run record is an effect
    /// of `variant` or an event of `variant`.
    fn run_records(&self, operation: &str, kind: &str, variant: &str) -> usize {
        self.journal("revisionRecord")
            .iter()
            .filter(|value| value.get("operationId").and_then(Json::as_str) == Some(operation))
            .filter_map(|value| value.get("record"))
            .filter(|record| record.get("kind").and_then(Json::as_str) == Some(kind))
            .filter(|record| {
                let value = record.get("value");
                let found = match kind {
                    "effect" => value.and_then(|v| v.get("variant")),
                    _ => value
                        .and_then(|v| v.get("fields"))
                        .and_then(|f| f.get("kind"))
                        .and_then(|k| k.get("variant")),
                };
                found.and_then(Json::as_str) == Some(variant)
            })
            .count()
    }

    /// Every live process whose executable is `name` in this state's
    /// generations.
    fn servers(&self, name: &str) -> Vec<i64> {
        let generations = self.state.join("revisions");
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
                    && executable
                        .file_name()
                        .is_some_and(|file| file.to_string_lossy().starts_with(name)))
                .then_some(pid)
            })
            .collect();
        found.sort();
        found
    }
}

fn plan_of(path: &Path) -> ignite::apply::plan::Plan {
    ignite::apply::plan::Plan::parse(&Json::parse(&read(path)).unwrap()).unwrap()
}

fn server_name(plan: &ignite::apply::plan::Plan) -> String {
    plan.artifacts
        .get("Terminal.server")
        .expect("server artifact")
        .name
        .clone()
}

/// The one owned, ready server of a converged status at `revision`.
fn converged(status: &Json, revision: i64) -> Option<i64> {
    let owned = status.get("ownership").and_then(Json::as_object)?;
    let ready = owned.len() == 1
        && owned
            .values()
            .all(|owner| owner.get("ready").and_then(Json::as_bool) == Some(true));
    (ready
        && status.get("committedRevision").and_then(Json::as_i64) == Some(revision)
        && status.get("activeOperation") == Some(&Json::Null))
    .then(|| owned.values().next()?.get("pid").and_then(Json::as_i64))
    .flatten()
}

fn await_converged(flow: &Flow, host: &Host, revision: i64) -> i64 {
    let mut pid = None;
    until(
        "convergence",
        || {
            pid = converged(&flow.inspect(), revision);
            pid.is_some()
        },
        || format!("{:?}\n{}", flow.inspect(), read(&host.stderr)),
    );
    pid.unwrap()
}

/// Send `status` to the terminal on `port` and return the reply.
fn terminal_status(port: u16) -> String {
    let mut stream = TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), LIVE_TIMEOUT)
        .expect("connect to the applied server");
    stream.set_read_timeout(Some(LIVE_TIMEOUT)).unwrap();
    let prompt = |stream: &mut TcpStream| {
        let deadline = Instant::now() + LIVE_TIMEOUT;
        let mut received = Vec::new();
        while !received.ends_with(b"terminal> \xff\xf9") {
            assert!(Instant::now() < deadline, "prompt deadline: {received:?}");
            let mut bytes = [0; 1024];
            let count = stream.read(&mut bytes).expect("read the terminal");
            assert!(count > 0, "server closed before prompt: {received:?}");
            received.extend_from_slice(&bytes[..count]);
        }
        String::from_utf8_lossy(&received).into_owned()
    };
    prompt(&mut stream);
    stream.write_all(b"status\r\n").unwrap();
    let reply = prompt(&mut stream);
    let _ = stream.write_all(b"quit\r\n");
    reply
}

/// Rows 10 and 11: a host that crashes after durable acceptance, after an
/// effect intent and after its dispatch is restarted each time; a process
/// of the owned generation the journal cannot account for blocks recovery as
/// `outcome_unknown`, is neither signalled nor adopted, and once it ended a
/// `status` resumes the operation, which completes with one server. Every
/// retry attaches to the one operation.
#[test]
fn crash_after_intent_before_dispatch_yields_one_server_or_outcome_unknown() {
    crash_after_intent("rust");
}

#[test]
fn crash_after_intent_before_dispatch_yields_one_server_or_outcome_unknown_llvm() {
    crash_after_intent("llvm");
}

fn crash_after_intent(target: &'static str) {
    let flow = Flow::for_target("intent", target);
    let mut host = Host::start(&flow, "after-intent", &[(FAULT, "after_intent")]);
    let saved = flow.prepare("first");
    let plan = plan_of(&saved);
    let name = server_name(&plan);
    flow.apply_fails("apply-crashes-after-intent", &saved);
    host.crashed("after_intent");
    let journaled = flow.journal("state").pop().unwrap();
    assert_eq!(
        journaled.get("activeOperation").and_then(Json::as_str),
        Some(plan.id.as_str()),
        "acceptance was durable before the crash: {journaled:?}"
    );
    assert_eq!(flow.run_records(&plan.id, "effect", "Spawn"), 0);
    assert!(flow.servers(&name).is_empty());

    let mut host = Host::spawn(&flow, "before-dispatch", &[(FAULT, "before_dispatch")]);
    host.crashed("before_dispatch");
    assert_eq!(flow.run_records(&plan.id, "effect", "Spawn"), 1);
    assert_eq!(flow.run_records(&plan.id, "event", "Started"), 0);
    assert!(
        flow.servers(&name).is_empty(),
        "the intent was not dispatched"
    );

    let mut host = Host::spawn(&flow, "after-dispatch", &[(FAULT, "after_dispatch")]);
    host.crashed("after_dispatch");
    assert_eq!(flow.run_records(&plan.id, "effect", "Spawn"), 2);
    assert_eq!(flow.run_records(&plan.id, "event", "Started"), 0);
    until(
        "the dispatched child ends with its host (parent-death rule)",
        || flow.servers(&name).is_empty(),
        || format!("{:?}", flow.servers(&name)),
    );
    let reconciled = flow.journal("reconciled");
    assert!(
        reconciled.iter().any(|value| value
            .get("intents")
            .and_then(Json::as_array)
            .is_some_and(|intents| !intents.is_empty())),
        "the undispatched intent was reconciled: {reconciled:?}"
    );

    // A process of the owned generation that no record accounts for: the
    // host cannot prove it is the dispatched instance or a foreign one.
    let generation = fs::read_dir(flow.state.join("revisions"))
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("bin/1").join(&name))
        .find(|path| path.is_file())
        .expect("a generation executable");
    let unproven_port = free_port();
    let mut unproven = Owned(
        Command::new(&generation)
            .args(["--port", &unproven_port.to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let unproven_pid = i64::from(unproven.0.id());
    until(
        "the unproven process runs from the generation",
        || flow.servers(&name) == vec![unproven_pid],
        || format!("{:?}", flow.servers(&name)),
    );

    let mut host = Host::start(&flow, "unknown", &[]);
    let status = flow.inspect();
    assert_eq!(
        status.get("recoveryReason").and_then(Json::as_str),
        Some("outcome_unknown"),
        "{status:?}"
    );
    let evidence = status
        .get("recovery")
        .and_then(|recovery| recovery.get("evidence"))
        .map(Json::compact)
        .unwrap_or_default();
    assert!(
        evidence.contains(&unproven_pid.to_string()),
        "the evidence names the unproven process: {status:?}"
    );
    assert_eq!(
        status
            .get("ownership")
            .and_then(Json::as_object)
            .map(|owned| owned.len()),
        Some(0)
    );
    let (code, _) = flow.apply_fails("retry-while-unknown", &saved);
    assert_eq!(code, Some(1), "an unknown outcome blocks the operation");
    std::thread::sleep(Duration::from_millis(500));
    assert!(
        unproven.0.try_wait().unwrap().is_none(),
        "an unproven process is never signalled"
    );
    assert_eq!(
        flow.servers(&name),
        vec![unproven_pid],
        "no duplicate spawn"
    );

    unproven.0.kill().unwrap();
    unproven.0.wait().unwrap();
    host.line("status");
    let pid = await_converged(&flow, &host, 1);
    let receipt = flow.apply("retry-after-recovery", &saved);
    assert_eq!(
        receipt.get("operationId").and_then(Json::as_str),
        Some(plan.id.as_str())
    );
    assert_eq!(receipt.get("revision").and_then(Json::as_i64), Some(1));
    assert_eq!(
        receipt.get("state").and_then(Json::as_str),
        Some("completed")
    );
    assert_eq!(flow.servers(&name), vec![pid], "exactly one server");
    assert!(terminal_status(flow.port).contains("ready"));
    host.stop();
}

/// Change the console's `status` reply, which replaces the server without
/// changing the model.
fn change_command_body(flow: &Flow) {
    let commands = flow.source.parent().unwrap().join("commands.ignite");
    let original = read(&commands);
    let before = "Status => Reply { lines = [\"ready\"] }";
    assert!(original.contains(before), "terminal fixture commands");
    fs::write(
        &commands,
        original.replace(
            before,
            "Status => Reply { lines = [\"replacement-ready\"] }",
        ),
    )
    .unwrap();
}

/// Rows 11 and 12: a host that crashes after journaling a transition step,
/// before its effects ran, is restarted and drives the desired revision to
/// one ready server; a later restart whose listener port a foreign process
/// holds neither signals nor adopts it and blocks as `foreign_listener`
/// until a `status` finds the port free.
#[test]
fn crash_mid_transition_resumes_once_and_a_foreign_listener_is_never_adopted() {
    crash_mid_transition("rust");
}

#[test]
fn crash_mid_transition_resumes_once_and_a_foreign_listener_is_never_adopted_llvm() {
    crash_mid_transition("llvm");
}

fn crash_mid_transition(target: &'static str) {
    let flow = Flow::for_target("transition", target);
    let mut host = Host::start(&flow, "first", &[]);
    let first = flow.prepare("first");
    let name = server_name(&plan_of(&first));
    flow.apply("first-apply", &first);
    let old = await_converged(&flow, &host, 1);
    host.stop();

    change_command_body(&flow);
    let mut host = Host::start(&flow, "mid-transition", &[(FAULT, "mid_transition")]);
    await_converged(&flow, &host, 1);
    let replacement = flow.prepare("replacement");
    let plan = plan_of(&replacement);
    let steps = flow.journal("transition").len();
    flow.apply_fails("apply-crashes-mid-transition", &replacement);
    host.crashed("mid_transition");
    assert_eq!(
        flow.journal("transition").len(),
        steps + 1,
        "the crash follows the first journaled step"
    );
    until(
        "the committed instance ends with its host",
        || flow.servers(&name).is_empty(),
        || format!("{:?}", flow.servers(&name)),
    );

    let mut host = Host::start(&flow, "resumed", &[]);
    let pid = await_converged(&flow, &host, 2);
    assert_ne!(pid, old);
    assert_eq!(flow.servers(&name), vec![pid], "one server generation");
    assert!(terminal_status(flow.port).contains("replacement-ready"));
    let receipt = flow.apply("retry-after-resume", &replacement);
    assert_eq!(
        receipt.get("operationId").and_then(Json::as_str),
        Some(plan.id.as_str())
    );
    assert_eq!(receipt.get("revision").and_then(Json::as_i64), Some(2));
    host.stop();
    until(
        "the server ends with its stopped host",
        || flow.servers(&name).is_empty(),
        || format!("{:?}", flow.servers(&name)),
    );

    let foreign = bind_when_free(flow.port);
    let mut host = Host::start(&flow, "foreign", &[]);
    let status = flow.inspect();
    assert_eq!(
        status.get("recoveryReason").and_then(Json::as_str),
        Some("foreign_listener"),
        "{status:?}"
    );
    std::thread::sleep(Duration::from_millis(500));
    assert!(flow.servers(&name).is_empty(), "nothing started or adopted");
    assert_eq!(
        foreign.local_addr().unwrap().port(),
        flow.port,
        "the foreign listener is untouched"
    );
    drop(foreign);
    host.line("status");
    let pid = await_converged(&flow, &host, 2);
    assert_eq!(flow.servers(&name), vec![pid]);
    assert!(terminal_status(flow.port).contains("replacement-ready"));
    host.stop();
}

/// Set the server's `maximum_attempts` from `from` to `to`.
fn set_attempts(flow: &Flow, from: i64, to: i64) {
    let source = read(&flow.source);
    let before = format!("maximum_attempts = {from}");
    assert!(source.contains(&before), "terminal fixture attempts");
    fs::write(
        &flow.source,
        source.replace(&before, &format!("maximum_attempts = {to}")),
    )
    .unwrap();
}

/// D3 and 17.5: a corrective apply after `gave_up` whose acceptance was
/// durable when the host crashed is resumed by the next host and
/// completes; it never stays `accepted` behind the `gave_up` it resolved.
#[test]
fn a_crash_after_accepting_a_corrective_resumes_the_corrective() {
    let flow = Flow::new("corrective");
    set_attempts(&flow, 10, 1);
    let occupied = TcpListener::bind(("127.0.0.1", flow.port)).expect("occupy the server port");
    let mut host = Host::start(&flow, "gives-up", &[]);
    let failing = flow.prepare("failing");
    let (code, _) = flow.apply_fails("apply-gives-up", &failing);
    assert_eq!(code, Some(1), "a machine that gives up fails the operation");
    let status = flow.inspect();
    assert_eq!(
        status.get("recoveryReason").and_then(Json::as_str),
        Some("gave_up"),
        "{status:?}"
    );
    host.stop();
    drop(occupied);

    set_attempts(&flow, 1, 10);
    let mut host = Host::start(&flow, "after-intent", &[(FAULT, "after_intent")]);
    let corrective = flow.prepare("corrective");
    let plan = plan_of(&corrective);
    flow.apply_fails("apply-corrective-crashes-after-intent", &corrective);
    host.crashed("after_intent");

    let mut host = Host::start(&flow, "resumed", &[]);
    let pid = await_converged(&flow, &host, 1);
    let receipt = flow.apply("retry-after-resume", &corrective);
    assert_eq!(
        receipt.get("operationId").and_then(Json::as_str),
        Some(plan.id.as_str())
    );
    assert_eq!(
        receipt.get("state").and_then(Json::as_str),
        Some("completed")
    );
    assert_eq!(receipt.get("revision").and_then(Json::as_i64), Some(1));
    assert_eq!(flow.inspect().get("recoveryReason"), Some(&Json::Null));
    assert_eq!(flow.servers(&server_name(&plan)), vec![pid]);
    assert!(terminal_status(flow.port).contains("ready"));
    host.stop();
}

/// Row 13: a journal sync that fails, of the acceptance or of an effect
/// intent, stops the host before the operation it records goes on; no
/// effect runs, and the next host recovers from the synced journal.
#[test]
fn a_failed_journal_sync_stops_the_host_before_any_dependent_effect() {
    let flow = Flow::new("sync");
    let mut host = Host::start(&flow, "sync-intent", &[(FAULT, "sync_intent")]);
    let saved = flow.prepare("first");
    let plan = plan_of(&saved);
    let name = server_name(&plan);
    let (code, _) = flow.apply_fails("apply-intent-not-durable", &saved);
    assert_eq!(code, Some(3), "a storage failure is exit 3");
    let ended = host.ended();
    assert_eq!(ended.code(), Some(3), "{}", read(&host.stderr));
    assert!(
        read(&host.stderr).contains("not durable"),
        "{}",
        read(&host.stderr)
    );
    assert!(
        flow.journal("state")
            .iter()
            .all(|state| state.get("activeOperation").and_then(Json::as_str) != Some(&plan.id)),
        "the acceptance whose sync failed is not in the journal"
    );

    let mut host = Host::start(&flow, "sync-effect", &[(FAULT, "sync_effect")]);
    let (code, _) = flow.apply_fails("apply-effect-not-durable", &saved);
    assert_ne!(code, Some(0));
    let ended = host.ended();
    assert_eq!(ended.code(), Some(3), "{}", read(&host.stderr));
    assert!(
        read(&host.stderr).contains("not durable"),
        "{}",
        read(&host.stderr)
    );
    assert_eq!(
        flow.run_records(&plan.id, "effect", "Spawn"),
        0,
        "the intent whose sync failed is not in the journal"
    );
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        flow.servers(&name).is_empty(),
        "the effect was not dispatched"
    );

    let mut host = Host::start(&flow, "recovered", &[]);
    let pid = await_converged(&flow, &host, 1);
    let receipt = flow.apply("retry-after-storage-failure", &saved);
    assert_eq!(
        receipt.get("operationId").and_then(Json::as_str),
        Some(plan.id.as_str())
    );
    assert_eq!(receipt.get("revision").and_then(Json::as_i64), Some(1));
    assert_eq!(flow.servers(&name), vec![pid]);
    host.stop();
}

/// Decision D2 and row 14: a host whose reducer revision differs from the
/// committed revision's starts, re-steps no segment of that history, drives
/// nothing, and reports `reducer_revision_changed` with the next action
/// "replan and apply"; the replanned apply starts the members afresh, and a
/// host of the same reducer revision re-steps every segment again.
#[test]
fn a_changed_reducer_revision_starts_the_host_without_driving_and_asks_to_replan() {
    let flow = Flow::new("reducer");
    let mut host = Host::start(&flow, "first", &[]);
    let first = flow.prepare("first");
    let name = server_name(&plan_of(&first));
    flow.apply("first-apply", &first);
    await_converged(&flow, &host, 1);
    host.stop();

    let mut host = Host::start(
        &flow,
        "changed-reducer",
        &[(
            "IGNITE_DEBUG_HOST_REDUCER_REVISION",
            "sha256:another-reducer",
        )],
    );
    let status = flow.inspect();
    assert_eq!(
        status.get("recoveryReason").and_then(Json::as_str),
        Some("reducer_revision_changed"),
        "{status:?}"
    );
    let recovery = status.get("recovery").expect("recovery");
    assert_eq!(
        recovery.get("state").and_then(Json::as_str),
        Some("recovery_required")
    );
    assert_eq!(
        recovery.get("nextAction").and_then(Json::as_str),
        Some("replan and apply")
    );
    let history = status.get("history").and_then(Json::as_array).unwrap();
    assert!(
        !history.is_empty()
            && history
                .iter()
                .all(|entry| entry.get("restepped") == Some(&Json::Bool(false))),
        "{history:?}"
    );
    std::thread::sleep(Duration::from_millis(500));
    assert!(flow.servers(&name).is_empty(), "the revision is not driven");
    assert_eq!(
        status
            .get("ownership")
            .and_then(Json::as_object)
            .map(|owned| owned.len()),
        Some(0)
    );

    change_command_body(&flow);
    let replanned = flow.prepare("replanned");
    let receipt = flow.apply("replanned-apply", &replanned);
    assert_eq!(receipt.get("revision").and_then(Json::as_i64), Some(2));
    let pid = await_converged(&flow, &host, 2);
    assert_eq!(flow.servers(&name), vec![pid]);
    assert_eq!(flow.inspect().get("recoveryReason"), Some(&Json::Null));
    host.stop();

    let mut host = Host::start(&flow, "same-reducer", &[]);
    await_converged(&flow, &host, 2);
    let status = flow.inspect();
    let history = status.get("history").and_then(Json::as_array).unwrap();
    assert!(
        history
            .iter()
            .filter(|entry| entry.get("planId").and_then(Json::as_str).is_some())
            .all(|entry| entry.get("restepped") == Some(&Json::Bool(true))),
        "{history:?}"
    );
    assert!(terminal_status(flow.port).contains("replacement-ready"));
    host.stop();
}

/// Copy the directory tree `from` to `to`.
fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let target = to.join(path.file_name().unwrap());
        if path.is_dir() {
            copy_tree(&path, &target);
        } else {
            fs::copy(&path, &target).unwrap();
        }
    }
}

/// 17.5, the precedence of recovery reasons: a host whose active operation
/// was recorded under another reducer revision (not driven,
/// `reducer_revision_changed`) and whose committed revision cannot resume
/// because a foreign process holds its listener port reports the first
/// reason and keeps the `foreign_listener` block beside it in `recovery`.
/// The block still blocks; once `status` finds the port free the committed
/// revision resumes and the report stands until a commit.
#[test]
fn a_foreign_listener_block_keeps_the_reducer_revision_changed_report() {
    let flow = Flow::new("precedence");
    let mut host = Host::start(&flow, "first", &[]);
    let first = flow.prepare("first");
    let committed = plan_of(&first);
    let name = server_name(&committed);
    flow.apply("first-apply", &first);
    await_converged(&flow, &host, 1);
    host.stop();

    let std = flow.root.join("std");
    copy_tree(&repo().join("std"), &std);
    let control = std.join("control.ignite");
    fs::write(&control, read(&control) + "// another reducer revision\n").unwrap();
    change_command_body(&flow);
    let mut host = Host::start(&flow, "mid-transition", &[(FAULT, "mid_transition")]);
    await_converged(&flow, &host, 1);
    let second = flow.prepare_with("other-reducer", &["--std", std.to_str().unwrap()]);
    let changed = plan_of(&second);
    assert_ne!(changed.reducer_revision, committed.reducer_revision);
    flow.apply_fails("apply-crashes-mid-transition", &second);
    host.crashed("mid_transition");
    let changed_name = server_name(&changed);
    until(
        "the instances end with their host",
        || flow.servers(&name).is_empty() && flow.servers(&changed_name).is_empty(),
        || format!("{:?}", flow.servers(&name)),
    );

    let foreign = TcpListener::bind(("127.0.0.1", flow.port)).expect("a foreign listener");
    let reducer = [(
        "IGNITE_DEBUG_HOST_REDUCER_REVISION",
        committed.reducer_revision.as_str(),
    )];
    let mut host = Host::start(&flow, "blocked", &reducer);
    let status = flow.inspect();
    let recovery = status.get("recovery").cloned().unwrap_or(Json::Null);
    let field = |value: &Json, key: &str| value.get(key).and_then(Json::as_str).map(str::to_string);
    assert_eq!(
        field(&status, "recoveryReason").as_deref(),
        Some("reducer_revision_changed"),
        "{}",
        status.compact()
    );
    assert_eq!(
        field(&recovery, "reason").as_deref(),
        Some("reducer_revision_changed"),
        "the block replaced the first reason: {}",
        recovery.compact()
    );
    assert_eq!(field(&recovery, "planId"), Some(changed.id.clone()));
    assert_eq!(
        recovery.get("blocking"),
        Some(&Json::Bool(true)),
        "the block still blocks: {}",
        recovery.compact()
    );
    let later = recovery
        .get("laterReasons")
        .and_then(Json::as_array)
        .map(<[Json]>::to_vec)
        .unwrap_or_default();
    assert_eq!(later.len(), 1, "{}", recovery.compact());
    assert_eq!(
        field(&later[0], "reason").as_deref(),
        Some("foreign_listener")
    );
    assert_eq!(field(&later[0], "planId"), Some(committed.id.clone()));
    let receipt = status
        .get("operations")
        .and_then(|operations| operations.get(&changed.id))
        .cloned()
        .unwrap_or(Json::Null);
    assert_eq!(
        field(&receipt, "reason").as_deref(),
        Some("reducer_revision_changed")
    );
    std::thread::sleep(Duration::from_millis(500));
    assert!(flow.servers(&name).is_empty(), "nothing started or adopted");

    drop(foreign);
    host.line("status");
    let pid = await_converged(&flow, &host, 1);
    assert_eq!(flow.servers(&name), vec![pid]);
    let status = flow.inspect();
    let recovery = status.get("recovery").cloned().unwrap_or(Json::Null);
    assert_eq!(
        field(&recovery, "reason").as_deref(),
        Some("reducer_revision_changed"),
        "lifting the block keeps the report: {}",
        status.compact()
    );
    assert_eq!(recovery.get("blocking"), Some(&Json::Bool(false)));
    assert_eq!(recovery.get("laterReasons"), None);
    host.stop();
}

/// 17.5: "Lifting the block keeps the report, no longer blocking, until the
/// next commit". A host stop and start is not a commit (17.5: "Stopping or
/// restarting a host is not a new apply"), so the `reducer_revision_changed`
/// report the lift kept must still stand after one.
#[test]
fn adversary_the_lifted_report_survives_a_host_restart_until_a_commit() {
    let flow = Flow::new("lifted-restart");
    let mut host = Host::start(&flow, "first", &[]);
    let first = flow.prepare("first");
    let committed = plan_of(&first);
    let name = server_name(&committed);
    flow.apply("first-apply", &first);
    await_converged(&flow, &host, 1);
    host.stop();

    let std = flow.root.join("std");
    copy_tree(&repo().join("std"), &std);
    let control = std.join("control.ignite");
    fs::write(&control, read(&control) + "// another reducer revision\n").unwrap();
    change_command_body(&flow);
    let mut host = Host::start(&flow, "mid-transition", &[(FAULT, "mid_transition")]);
    await_converged(&flow, &host, 1);
    let second = flow.prepare_with("other-reducer", &["--std", std.to_str().unwrap()]);
    let changed = plan_of(&second);
    flow.apply_fails("apply-crashes-mid-transition", &second);
    host.crashed("mid_transition");
    let changed_name = server_name(&changed);
    until(
        "the instances end with their host",
        || flow.servers(&name).is_empty() && flow.servers(&changed_name).is_empty(),
        || format!("{:?}", flow.servers(&name)),
    );

    let foreign = TcpListener::bind(("127.0.0.1", flow.port)).expect("a foreign listener");
    let reducer = [(
        "IGNITE_DEBUG_HOST_REDUCER_REVISION",
        committed.reducer_revision.as_str(),
    )];
    let mut host = Host::start(&flow, "blocked", &reducer);
    drop(foreign);
    host.line("status");
    await_converged(&flow, &host, 1);
    let lifted = flow.inspect();
    let reason = |status: &Json| {
        status
            .get("recovery")
            .and_then(|recovery| recovery.get("reason"))
            .and_then(Json::as_str)
            .map(str::to_string)
    };
    assert_eq!(
        reason(&lifted).as_deref(),
        Some("reducer_revision_changed"),
        "the lift keeps the report: {}",
        lifted.compact()
    );
    host.stop();

    let mut host = Host::start(&flow, "restarted", &reducer);
    await_converged(&flow, &host, 1);
    let restarted = flow.inspect();
    host.stop();
    assert_eq!(
        restarted.get("committedRevision").and_then(Json::as_i64),
        Some(1),
        "no commit happened"
    );
    assert_eq!(
        reason(&restarted).as_deref(),
        Some("reducer_revision_changed"),
        "the report kept by the lift vanished at a restart before any commit: {}",
        restarted.compact()
    );
    assert_eq!(
        restarted.get("recoveryReason").and_then(Json::as_str),
        Some("reducer_revision_changed")
    );
}
