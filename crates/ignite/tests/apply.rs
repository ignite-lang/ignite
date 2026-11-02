//! Local apply through the real CLI, host socket, compiler and native program.

use std::cell::Cell;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ignite_eval::Json;

const LIVE_TIMEOUT: Duration = Duration::from_secs(15);
const BUILD_TIMEOUT: Duration = Duration::from_secs(120);
static APPLY_BUILDS: Mutex<()> = Mutex::new(());

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

struct Host {
    child: Option<Child>,
    stderr: PathBuf,
}

impl Host {
    fn start(flow: &Flow, label: &str) -> Host {
        let stderr = flow.root.join(format!("{label}.stderr"));
        let child = Command::new(env!("CARGO_BIN_EXE_ignite"))
            .args(["host", "--state"])
            .arg(&flow.state)
            .current_dir(repo())
            .env("CARGO_INCREMENTAL", "0")
            .env("CARGO_PROFILE_DEV_DEBUG", "line-tables-only")
            .stdin(Stdio::piped())
            .stdout(File::create(flow.root.join(format!("{label}.stdout"))).unwrap())
            .stderr(File::create(&stderr).unwrap())
            .spawn()
            .expect("start foreground host");
        let mut host = Host {
            child: Some(child),
            stderr,
        };
        let deadline = Instant::now() + LIVE_TIMEOUT;
        loop {
            assert!(
                host.child.as_mut().unwrap().try_wait().unwrap().is_none(),
                "host exited: {}",
                read(&host.stderr)
            );
            if ignite::apply::inspect(&flow.state, None)
                .is_ok_and(|status| status.get("online").and_then(Json::as_bool) == Some(true))
            {
                return host;
            }
            assert!(
                Instant::now() < deadline,
                "host socket deadline: {}",
                read(&host.stderr)
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn stop(&mut self) {
        let child = self.child.as_mut().expect("host is running");
        let stdin = child.stdin.as_mut().expect("host control stdin");
        writeln!(stdin, "stop").unwrap();
        stdin.flush().unwrap();
        let status = wait(child, Duration::from_secs(20));
        assert!(
            status.is_some_and(|status| status.success()),
            "host stop failed: {status:?}\n{}",
            read(&self.stderr)
        );
        self.child.take();
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
            if wait(&mut child, Duration::from_secs(5)).is_none() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

struct Flow {
    root: PathBuf,
    state: PathBuf,
    source: PathBuf,
    counter: Cell<u64>,
}

impl Flow {
    fn new(target: &str, port: u16) -> Flow {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("apply-{target}-{}-{stamp}", std::process::id()));
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
        assert!(
            original.contains("port = 2323"),
            "terminal fixture port changed"
        );
        fs::write(
            &source,
            original.replace("port = 2323", &format!("port = {port}")),
        )
        .unwrap();
        Flow {
            state: root.join("state"),
            root,
            source,
            counter: Cell::new(0),
        }
    }

    fn cli(&self, label: &str, args: &[&str], expected: i32) -> String {
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
        let status = match wait(&mut child, limit) {
            Some(status) => status,
            None => {
                let _ = child.kill();
                let _ = child.wait();
                panic!(
                    "{label} exceeded {limit:?}\nstdout: {}\nstderr: {}",
                    read(&stdout),
                    read(&stderr)
                );
            }
        };
        assert_eq!(
            status.code(),
            Some(expected),
            "{label}\nstdout: {}\nstderr: {}",
            read(&stdout),
            read(&stderr)
        );
        read(&stdout)
    }

    fn json(&self, label: &str, args: &[&str], expected: i32) -> Json {
        let output = self.cli(label, args, expected);
        Json::parse(&output).unwrap_or_else(|reason| panic!("{label}: {reason}\n{output}"))
    }

    fn prepare(&self, label: &str, target: &str) -> PathBuf {
        let output = self.root.join(format!("{label}.json"));
        let runtime = (target == "llvm").then(crate::common::runtime_archive);
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
            "--target",
            target,
        ];
        if let Some(runtime) = &runtime {
            args.extend(["--runtime", runtime.to_str().unwrap()]);
        }
        self.cli(label, &args, 0);
        ignite::apply::plan::Plan::parse(&Json::parse(&read(&output)).unwrap())
            .expect("saved versioned apply envelope");
        output
    }

    fn apply(&self, label: &str, plan: &Path) -> Json {
        self.json(
            label,
            &[
                "apply",
                plan.to_str().unwrap(),
                "--state",
                self.state.to_str().unwrap(),
            ],
            0,
        )
    }

    fn inspect(&self) -> Json {
        self.json(
            "inspect",
            &["inspect", "--state", self.state.to_str().unwrap(), "--json"],
            0,
        )
    }
}

fn owned_pid(status: &Json) -> i64 {
    let owned = status
        .get("ownership")
        .and_then(Json::as_object)
        .expect("ownership map");
    assert_eq!(owned.len(), 1, "{status:?}");
    let owner = owned.values().next().unwrap();
    assert_eq!(
        owner.get("ready").and_then(Json::as_bool),
        Some(true),
        "{owner:?}"
    );
    owner.get("pid").and_then(Json::as_i64).expect("owned PID")
}

fn terminal_prompt(port: u16) {
    let mut stream = TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), LIVE_TIMEOUT)
        .expect("connect to applied server");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let deadline = Instant::now() + LIVE_TIMEOUT;
    let mut received = Vec::new();
    while !received.ends_with(b"terminal> ") {
        assert!(
            Instant::now() < deadline,
            "terminal prompt deadline: {received:?}"
        );
        let mut bytes = [0; 1024];
        let count = stream
            .read(&mut bytes)
            .expect("read applied terminal prompt");
        assert!(count > 0, "server closed before prompt: {received:?}");
        received.extend_from_slice(&bytes[..count]);
    }
}

fn workflow(target: &str) {
    let _build_guard = APPLY_BUILDS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let reserved = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = reserved.local_addr().unwrap().port();
    let flow = Flow::new(target, port);
    let mut host = Host::start(&flow, "host");
    let first = flow.prepare("first", target);
    drop(reserved);
    let receipt = flow.apply("first-apply", &first);
    assert_eq!(
        receipt.get("state").and_then(Json::as_str),
        Some("completed")
    );
    assert_eq!(receipt.get("revision").and_then(Json::as_i64), Some(1));
    let status = flow.inspect();
    assert_eq!(
        status.get("committedRevision").and_then(Json::as_i64),
        Some(1)
    );
    let pid = owned_pid(&status);
    terminal_prompt(port);

    let retried = flow.apply("retry", &first);
    assert_eq!(retried.get("operationId"), receipt.get("operationId"));
    assert_eq!(retried.get("revision").and_then(Json::as_i64), Some(1));
    assert_eq!(owned_pid(&flow.inspect()), pid);

    let stale = flow.prepare("stale", target);
    let original = fs::read_to_string(&flow.source).unwrap();
    fs::write(
        &flow.source,
        format!("{original}\n// source-bound admission regression\n"),
    )
    .unwrap();
    flow.cli(
        "reject-stale-source",
        &[
            "apply",
            stale.to_str().unwrap(),
            "--state",
            flow.state.to_str().unwrap(),
        ],
        2,
    );
    assert_eq!(owned_pid(&flow.inspect()), pid);
    terminal_prompt(port);

    let noop = flow.prepare("comment-noop", target);
    let unchanged = flow.apply("apply-comment-noop", &noop);
    assert_eq!(unchanged.get("noop").and_then(Json::as_bool), Some(true));
    assert_eq!(unchanged.get("revision").and_then(Json::as_i64), Some(1));
    assert_eq!(owned_pid(&flow.inspect()), pid);

    host.stop();
    let offline = flow.inspect();
    assert_eq!(offline.get("online").and_then(Json::as_bool), Some(false));
    fs::write(&flow.source, "invalid workspace after accepted revision\n").unwrap();
    let mut restarted = Host::start(&flow, "restarted-host");
    let deadline = Instant::now() + LIVE_TIMEOUT;
    loop {
        let status = flow.inspect();
        let ready = status
            .get("ownership")
            .and_then(Json::as_object)
            .is_some_and(|owned| {
                owned.len() == 1
                    && owned
                        .values()
                        .all(|owner| owner.get("ready").and_then(Json::as_bool) == Some(true))
            });
        if ready {
            assert_eq!(
                status.get("committedRevision").and_then(Json::as_i64),
                Some(1)
            );
            break;
        }
        assert!(
            Instant::now() < deadline,
            "retained revision did not resume: {status:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    terminal_prompt(port);
    let historical = flow.apply("retry-after-restart", &first);
    assert_eq!(historical.get("operationId"), receipt.get("operationId"));
    assert_eq!(historical.get("revision").and_then(Json::as_i64), Some(1));
    restarted.stop();
}

#[test]
fn rust_saved_apply_retry_source_admission_and_retained_restart() {
    workflow("rust");
}

#[test]
fn llvm_saved_apply_retry_source_admission_and_retained_restart() {
    workflow("llvm");
}

struct Client(Child);

impl Drop for Client {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

#[test]
fn accepted_apply_recovers_after_host_crash_during_failed_dispatch() {
    let _build_guard = APPLY_BUILDS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let reserved = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = reserved.local_addr().unwrap().port();
    let flow = Flow::new("pending-recovery", port);
    let mut host = Host::start(&flow, "host");
    let saved = flow.prepare("pending", "rust");
    let plan = ignite::apply::plan::Plan::parse(&Json::parse(&read(&saved)).unwrap()).unwrap();
    let client_stdout = flow.root.join("pending-apply.stdout");
    let client_stderr = flow.root.join("pending-apply.stderr");
    let mut client = Client(
        Command::new(env!("CARGO_BIN_EXE_ignite"))
            .arg("apply")
            .arg(&saved)
            .arg("--state")
            .arg(&flow.state)
            .current_dir(repo())
            .stdin(Stdio::null())
            .stdout(File::create(&client_stdout).unwrap())
            .stderr(File::create(&client_stderr).unwrap())
            .spawn()
            .unwrap(),
    );
    let journal = flow.state.join("host-journal.jsonl");
    let deadline = Instant::now() + LIVE_TIMEOUT;
    loop {
        let records: Vec<Json> = read(&journal)
            .lines()
            .filter_map(|line| Json::parse(line).ok())
            .filter(|record| record.get("kind").and_then(Json::as_str) == Some("revisionRecord"))
            .filter(|record| {
                record
                    .get("value")
                    .and_then(|value| value.get("operationId"))
                    .and_then(Json::as_str)
                    == Some(plan.id.as_str())
            })
            .filter_map(|record| {
                record
                    .get("value")
                    .and_then(|value| value.get("record"))
                    .cloned()
            })
            .collect();
        let saw = |wanted: &str| {
            records.iter().any(|record| {
                record.get("kind").and_then(Json::as_str) == Some("event")
                    && record
                        .get("value")
                        .and_then(|value| value.get("fields"))
                        .and_then(|fields| fields.get("kind"))
                        .and_then(|kind| kind.get("variant"))
                        .and_then(Json::as_str)
                        == Some(wanted)
            })
        };
        if saw("Started") && saw("Exited") && read(&host.stderr).contains("restart attempt") {
            break;
        }
        assert!(
            client.0.try_wait().unwrap().is_none(),
            "apply ended before pending crash boundary\n{}\n{}",
            read(&client_stdout),
            read(&client_stderr)
        );
        assert!(
            Instant::now() < deadline,
            "no observed failed owned dispatch before deadline\n{}",
            read(&host.stderr)
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        flow.inspect()
            .get("committedRevision")
            .and_then(Json::as_i64),
        Some(0)
    );
    client
        .0
        .kill()
        .expect("disconnect the accepted apply client");
    assert!(!client.0.wait().unwrap().success());
    let deadline = Instant::now() + LIVE_TIMEOUT;
    let restarts_after_disconnect = read(&host.stderr).matches("restart attempt ").count();
    while read(&host.stderr).matches("restart attempt ").count() <= restarts_after_disconnect {
        assert!(
            host.child.as_mut().unwrap().try_wait().unwrap().is_none(),
            "host stopped after client disconnect: {}",
            read(&host.stderr)
        );
        assert!(
            Instant::now() < deadline,
            "accepted work stopped after client disconnect: {}",
            read(&host.stderr)
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let mut killed = host.child.take().unwrap();
    killed.kill().expect("SIGKILL the foreground host");
    assert!(!killed.wait().unwrap().success());
    let disconnected = wait(&mut client.0, LIVE_TIMEOUT).expect("apply client observes host crash");
    assert!(
        !disconnected.success(),
        "uncommitted apply cannot report completion"
    );
    drop(reserved);

    let mut resumed = Host::start(&flow, "resumed-host");
    let receipt = flow.apply("retry-pending-after-crash", &saved);
    assert_eq!(
        receipt.get("operationId").and_then(Json::as_str),
        Some(plan.id.as_str())
    );
    assert_eq!(receipt.get("revision").and_then(Json::as_i64), Some(1));
    assert_eq!(
        receipt.get("state").and_then(Json::as_str),
        Some("completed")
    );
    let status = flow.inspect();
    assert_eq!(
        status.get("committedRevision").and_then(Json::as_i64),
        Some(1)
    );
    let pid = owned_pid(&status);
    let server_name = &plan
        .artifacts
        .get("Terminal.server")
        .expect("server artifact")
        .name;
    let generation_root = flow.state.join("revisions");
    let running_servers: Vec<i64> = fs::read_dir("/proc")
        .unwrap()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let pid = entry.file_name().to_string_lossy().parse().ok()?;
            let executable = fs::read_link(entry.path().join("exe")).ok()?;
            (executable.starts_with(&generation_root)
                && executable
                    .file_name()
                    .is_some_and(|name| name == server_name.as_str()))
            .then_some(pid)
        })
        .collect();
    assert_eq!(
        running_servers,
        vec![pid],
        "one OS server process in the retained generations"
    );
    terminal_prompt(port);
    let retry = flow.apply("retry-recovered-completion", &saved);
    assert_eq!(retry.get("operationId"), receipt.get("operationId"));
    assert_eq!(retry.get("revision").and_then(Json::as_i64), Some(1));
    resumed.stop();
}

#[test]
fn tampered_host_history_is_rejected_before_startup() {
    let flow = Flow::new("tampered-history", 0);
    let mut host = Host::start(&flow, "host");
    host.stop();
    let journal = flow.state.join("host-journal.jsonl");
    let original = read(&journal);
    let mut records: Vec<Json> = original
        .lines()
        .map(|line| Json::parse(line).unwrap())
        .collect();
    let Json::Object(record) = records.first_mut().expect("durable initial host history") else {
        panic!("journal record object")
    };
    let Json::Object(value) = record.get_mut("value").expect("historical state value") else {
        panic!("journal state object")
    };
    value.insert("committedRevision".into(), Json::Int(999));
    fs::write(
        &journal,
        records
            .iter()
            .map(Json::compact)
            .collect::<Vec<_>>()
            .join("\n")
            + "\n",
    )
    .unwrap();
    flow.cli(
        "reject-tampered-host",
        &["host", "--state", flow.state.to_str().unwrap()],
        3,
    );
    let sequence = flow.counter.get() - 1;
    let stderr = flow
        .root
        .join(format!("{sequence}-reject-tampered-host.stderr"));
    assert!(
        read(&stderr).contains("digest mismatch"),
        "{}",
        read(&stderr)
    );
    assert_eq!(
        flow.inspect().get("online").and_then(Json::as_bool),
        Some(false)
    );
}

struct TerminalSession(TcpStream);

impl TerminalSession {
    fn connect(port: u16) -> TerminalSession {
        let stream =
            TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), LIVE_TIMEOUT).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut session = TerminalSession(stream);
        session.prompt();
        session
    }

    fn prompt(&mut self) -> String {
        let deadline = Instant::now() + LIVE_TIMEOUT;
        let mut response = Vec::new();
        while !response.ends_with(b"terminal> ") {
            assert!(
                Instant::now() < deadline,
                "session prompt deadline: {response:?}"
            );
            let mut bytes = [0; 1024];
            let count = self
                .0
                .read(&mut bytes)
                .expect("read persistent terminal session");
            assert!(count > 0, "session ended before prompt: {response:?}");
            response.extend_from_slice(&bytes[..count]);
        }
        String::from_utf8_lossy(&response).into_owned()
    }

    fn status(&mut self, expected: &str) {
        self.0.write_all(b"status\r\n").unwrap();
        let response = self.prompt();
        assert!(
            response.lines().any(|line| line.trim() == expected),
            "expected status line {expected:?}: {response:?}"
        );
    }

    fn quit(mut self) {
        self.0.write_all(b"quit\r\n").unwrap();
    }
}

#[test]
fn changed_command_body_replaces_ready_server_while_old_sessions_drain() {
    let _build_guard = APPLY_BUILDS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let reserved = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = reserved.local_addr().unwrap().port();
    let flow = Flow::new("replacement", port);
    let mut host = Host::start(&flow, "host");
    let first = flow.prepare("first", "rust");
    let first_plan =
        ignite::apply::plan::Plan::parse(&Json::parse(&read(&first)).unwrap()).unwrap();
    drop(reserved);
    let receipt = flow.apply("first-apply", &first);
    assert_eq!(receipt.get("revision").and_then(Json::as_i64), Some(1));
    let old_pid = owned_pid(&flow.inspect());
    let mut old_session = TerminalSession::connect(port);
    old_session.status("ready");

    let commands = flow.source.parent().unwrap().join("commands.ignite");
    let original = read(&commands);
    assert!(original.contains("Status => Reply { lines = [\"ready\"] }"));
    fs::write(
        &commands,
        original.replace(
            "Status => Reply { lines = [\"ready\"] }",
            "Status => Reply { lines = [\"replacement-ready\"] }",
        ),
    )
    .unwrap();
    let replacement = flow.prepare("replacement", "rust");
    let replacement_plan =
        ignite::apply::plan::Plan::parse(&Json::parse(&read(&replacement)).unwrap()).unwrap();
    assert_eq!(
        ignite::apply::plan::semantic_model(&first_plan.model),
        ignite::apply::plan::semantic_model(&replacement_plan.model),
        "command-body change preserves the model"
    );
    assert_ne!(
        first_plan.operational_identity,
        replacement_plan.operational_identity
    );
    let stdout = flow.root.join("replacement-apply.stdout");
    let stderr = flow.root.join("replacement-apply.stderr");
    let mut client = Client(
        Command::new(env!("CARGO_BIN_EXE_ignite"))
            .arg("apply")
            .arg(&replacement)
            .arg("--state")
            .arg(&flow.state)
            .current_dir(repo())
            .stdin(Stdio::null())
            .stdout(File::create(&stdout).unwrap())
            .stderr(File::create(&stderr).unwrap())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + LIVE_TIMEOUT;
    let new_pid = loop {
        let status = flow.inspect();
        let ready_pid = status
            .get("ownership")
            .and_then(Json::as_object)
            .into_iter()
            .flat_map(|owned| owned.values())
            .filter(|owned| owned.get("ready").and_then(Json::as_bool) == Some(true))
            .filter_map(|owned| owned.get("pid").and_then(Json::as_i64))
            .find(|pid| *pid != old_pid);
        if let Some(pid) = ready_pid {
            break pid;
        }
        assert!(
            client.0.try_wait().unwrap().is_none(),
            "replacement finished before new server readiness\n{}\n{}",
            read(&stdout),
            read(&stderr)
        );
        assert!(
            Instant::now() < deadline,
            "replacement server readiness deadline: {status:?}\n{}",
            read(&host.stderr)
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    let mut new_session = TerminalSession::connect(port);
    new_session.status("replacement-ready");
    old_session.status("ready");
    assert!(
        client.0.try_wait().unwrap().is_none(),
        "apply must wait for the old session to drain"
    );
    old_session.quit();
    let result =
        wait(&mut client.0, LIVE_TIMEOUT).expect("replacement completes after old session closes");
    assert!(
        result.success(),
        "replacement apply: {}\n{}",
        read(&stdout),
        read(&stderr)
    );
    let completed = Json::parse(&read(&stdout)).unwrap();
    assert_eq!(completed.get("revision").and_then(Json::as_i64), Some(2));
    assert_eq!(
        completed.get("state").and_then(Json::as_str),
        Some("completed")
    );
    assert_eq!(owned_pid(&flow.inspect()), new_pid);
    let generation_root = flow.state.join("revisions");
    let server_name = &replacement_plan
        .artifacts
        .get("Terminal.server")
        .unwrap()
        .name;
    let servers: Vec<i64> = fs::read_dir("/proc")
        .unwrap()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let pid = entry.file_name().to_string_lossy().parse().ok()?;
            let executable = fs::read_link(entry.path().join("exe")).ok()?;
            (executable.starts_with(&generation_root)
                && executable
                    .file_name()
                    .is_some_and(|name| name == server_name.as_str()))
            .then_some(pid)
        })
        .collect();
    assert_eq!(
        servers,
        vec![new_pid],
        "old OS process retires after draining"
    );
    let historical = flow.apply("historical-first-plan-retry", &first);
    assert_eq!(historical.get("operationId"), receipt.get("operationId"));
    assert_eq!(historical.get("revision").and_then(Json::as_i64), Some(1));
    assert_eq!(
        historical.get("currentRevision").and_then(Json::as_i64),
        Some(2)
    );
    assert_eq!(owned_pid(&flow.inspect()), new_pid);
    new_session.status("replacement-ready");
    new_session.quit();
    host.stop();
    fs::write(&commands, "invalid workspace after committed replacement\n").unwrap();
    let mut restored = Host::start(&flow, "replacement-restarted");
    let deadline = Instant::now() + LIVE_TIMEOUT;
    loop {
        let status = flow.inspect();
        if status
            .get("ownership")
            .and_then(Json::as_object)
            .is_some_and(|owned| {
                owned.len() == 1
                    && owned
                        .values()
                        .all(|owner| owner.get("ready").and_then(Json::as_bool) == Some(true))
            })
        {
            assert_eq!(
                status.get("committedRevision").and_then(Json::as_i64),
                Some(2)
            );
            break;
        }
        assert!(
            Instant::now() < deadline,
            "committed replacement did not resume: {status:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut restored_session = TerminalSession::connect(port);
    restored_session.status("replacement-ready");
    restored_session.quit();
    restored.stop();
}
