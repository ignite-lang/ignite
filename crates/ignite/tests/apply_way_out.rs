//! `ignite recover` (`docs/slice.md` 22.9, 22.0 rows 25 and 26): the
//! operator's way back to accepting plans from a recovery that no
//! corrective plan resolves, through the real CLI, host socket and native
//! program on the terminal design, its probe and contracts kept.
//!
//! The state both rows drive (22.9): revision 1 committed; revision 2 adds
//! a second server on a held port, which exits at once, so its supervisor
//! gives up (`gave_up`); then the running revision's copy of `terminald` is
//! replaced with other bytes and the server SIGKILLed, so its restart is a
//! later reason no corrective resolves.

use std::cell::Cell;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ignite_eval::Json;

use crate::common::apply::{BUILD as BUILD_TIMEOUT, LIVE as LIVE_TIMEOUT};

// The deadlines of this file besides `LIVE_TIMEOUT` and `BUILD_TIMEOUT`.

/// How long a `Host` dropped without `stop` gets to stop before it is
/// killed; cleanup, no assertion.
const STOP_ON_DROP: Duration = Duration::from_secs(10);
/// How long the start of the copied `sleep` retries while the copy is still
/// open for writing elsewhere (`ExecutableFileBusy`).
const BUSY_RETRY: Duration = Duration::from_secs(2);

const ARTIFACT_DIFFERS: &str = "ready process artifact differs from the admitted desired artifact";

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

fn alive(pid: i64) -> bool {
    fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|stat| {
        stat.rsplit_once(") ")
            .and_then(|(_, fields)| fields.split_whitespace().next())
            .is_some_and(|state| !matches!(state, "Z" | "X"))
    })
}

struct Host {
    child: Option<Child>,
    stderr: PathBuf,
}

impl Host {
    /// Start a host on `flow`'s state and wait until it answers.
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

    fn stop(&mut self) {
        let mut child = self.child.take().expect("host is running");
        let stdin = child.stdin.as_mut().unwrap();
        writeln!(stdin, "stop").unwrap();
        stdin.flush().unwrap();
        let status = wait(&mut child, LIVE_TIMEOUT);
        if status.is_none() {
            let _ = child.kill();
            let _ = child.wait();
        }
        assert!(
            status.is_some_and(|status| status.success()),
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
    /// A fresh copy of the terminal design on a free port, built for
    /// `target`, and a fresh state whose `build` is the one directory every
    /// apply test shares (O41).
    fn new(label: &str, target: &'static str) -> Flow {
        let port = free_port();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "way-out-{label}-{target}-{}-{stamp}",
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
        let original = read(&source);
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
            root: fs::canonicalize(&root).unwrap(),
            state: fs::canonicalize(&state).unwrap(),
            source,
            port,
            target,
            counter: Cell::new(0),
        }
    }

    /// Run the CLI with `args`: its exit code, stdout and stderr.
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
        let _build = crate::common::apply::build_lock(self.target);
        self.cli(label, &args, 0);
        output
    }

    /// `ignite apply` of `plan`: exit code and stderr.
    fn apply(&self, label: &str, plan: &Path) -> (Option<i32>, String) {
        let (code, _, stderr) = self.run(
            label,
            &[
                "apply",
                plan.to_str().unwrap(),
                "--state",
                self.state.to_str().unwrap(),
            ],
        );
        (code, stderr)
    }

    /// `ignite recover` of `operation`: exit code, stdout and stderr.
    fn recover(&self, label: &str, operation: &str) -> (Option<i32>, String, String) {
        self.run(
            label,
            &[
                "recover",
                "--state",
                self.state.to_str().unwrap(),
                "--operation",
                operation,
            ],
        )
    }

    fn inspect(&self) -> Json {
        ignite::apply::inspect(&self.state, None).expect("inspect")
    }

    /// Every host journal record: kind and value.
    fn journal(&self) -> Vec<(String, Json)> {
        read(&self.state.join("host-journal.jsonl"))
            .lines()
            .filter_map(|line| Json::parse(line).ok())
            .map(|record| {
                (
                    record
                        .get("kind")
                        .and_then(Json::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    record.get("value").cloned().unwrap_or(Json::Null),
                )
            })
            .collect()
    }

    /// Every live process whose executable is named `name` in this state's
    /// generations.
    fn servers(&self, name: &str) -> Vec<i64> {
        let generations = self.state.join("revisions");
        let mut found: Vec<i64> = fs::read_dir("/proc")
            .unwrap()
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let pid = entry.file_name().to_string_lossy().parse().ok()?;
                let executable = fs::read_link(entry.path().join("exe")).ok()?;
                (alive(pid)
                    && executable.starts_with(&generations)
                    && executable.file_name().is_some_and(|file| file == name))
                .then_some(pid)
            })
            .collect();
        found.sort();
        found
    }

    /// The revision state directory whose run realizes plan `id`.
    fn run_directory(&self, id: &str) -> PathBuf {
        fs::read_dir(self.state.join("revisions"))
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                Json::parse(&read(&path.join("context.json")))
                    .ok()
                    .and_then(|context| {
                        context
                            .get("planId")
                            .and_then(Json::as_str)
                            .map(str::to_string)
                    })
                    .as_deref()
                    == Some(id)
            })
            .unwrap_or_else(|| panic!("no revision state runs plan {id}"))
    }
}

fn plan_id(path: &Path) -> String {
    Json::parse(&read(path))
        .unwrap()
        .get("id")
        .and_then(Json::as_str)
        .expect("saved plan id")
        .to_string()
}

fn receipt(status: &Json, id: &str) -> Json {
    status
        .get("operations")
        .and_then(|operations| operations.get(id))
        .cloned()
        .unwrap_or(Json::Null)
}

fn field<'a>(value: &'a Json, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Json::as_str)
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
        || format!("{}\n{}", flow.inspect().compact(), read(&host.stderr)),
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

/// `source` with a second server on `port` that gives up after one
/// attempt; its ids sort before `ingress` and `server`, so the operation
/// that adds it starts it first.
fn with_auxiliary(source: &str, port: u16) -> String {
    let members = format!(
        r#"  resource altIngress: TcpListener {{
    address = "127.0.0.1"
    port = {port}
    exposure = Exposure.Loopback
    backlog = 128
  }}

  component auxConsole: CommandConsole {{
    implementation = module("examples.terminal.commands")
    prompt = "terminal> "
    maximum_line = 4096 bytes
    command_schema = module("examples.terminal.command_schema")
  }}

  component auxiliary: TelnetServer {{
    listener = altIngress
    application = auxConsole
    executable = "terminald2"
    recovery = Restart {{
      backoff = exponential(100 ms, 1 seconds)
      maximum_attempts = 1
      interval = 5 minutes
    }}
    rollout = DrainRollout {{
      deadline = 10 seconds
      on_deadline = "disconnect_remaining"
    }}
  }}

"#
    );
    let client = "  component client: Application";
    let binding = "    Terminal.client = workstationExecutable()\n";
    assert!(source.contains(client) && source.contains(binding));
    source.replace(client, &format!("{members}{client}")).replace(
        binding,
        &format!(
            "{binding}    Terminal.altIngress = localSocket()\n    Terminal.auxiliary = nativeProcess(\n      includes = [Terminal.auxConsole]\n    )\n"
        ),
    )
}

/// Change the console's `status` reply, which replaces the server.
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

/// The state of 22.9 on a running host: the committed revision 1, the
/// failed operation `gave_up` and a later reason no corrective resolves.
/// The failed operation's id and the reasons kept beside it.
struct Unresolvable {
    failed: String,
    failing_plan: PathBuf,
    later: Vec<String>,
    _occupied: TcpListener,
}

fn unresolvable(flow: &Flow, host: &Host) -> Unresolvable {
    let first = flow.prepare("first");
    let (code, stderr) = flow.apply("first-apply", &first);
    assert_eq!(code, Some(0), "{stderr}");
    let old = await_converged(flow, host, 1);

    let occupied = TcpListener::bind(("127.0.0.1", 0)).expect("hold the second server's port");
    let original = read(&flow.source);
    fs::write(
        &flow.source,
        with_auxiliary(&original, occupied.local_addr().unwrap().port()),
    )
    .unwrap();
    let failing_plan = flow.prepare("failing");
    let failed = plan_id(&failing_plan);
    let (code, stderr) = flow.apply("apply-gives-up", &failing_plan);
    assert_eq!(code, Some(1), "the second server gives up: {stderr}");
    let status = flow.inspect();
    assert_eq!(
        field(&status, "recoveryReason"),
        Some("gave_up"),
        "{}",
        status.compact()
    );
    assert_eq!(field(&status, "activeOperation"), Some(failed.as_str()));
    fs::write(&flow.source, original).unwrap();

    // The running revision's copy of `terminald` with other bytes, written
    // beside it and renamed over it, then the server killed: its restart
    // runs the other bytes.
    let bin = flow.run_directory(&failed).join("bin");
    let generation = fs::read_dir(&bin)
        .unwrap()
        .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse::<i64>().ok())
        .max()
        .expect("a generation of the running revision");
    let copy = bin.join(generation.to_string()).join("terminald");
    let mut bytes = fs::read(&copy).expect("the run's copy of terminald");
    bytes.extend_from_slice(b"\0other bytes");
    let beside = copy.with_file_name(".terminald.other");
    fs::write(&beside, &bytes).unwrap();
    fs::set_permissions(&beside, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    fs::rename(&beside, &copy).unwrap();
    let server = status
        .get("ownership")
        .and_then(|owned| owned.get("Terminal.server"))
        .and_then(|owner| owner.get("pid"))
        .and_then(Json::as_i64)
        .expect("the committed server");
    assert_eq!(server, old, "the committed server kept serving");
    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(server as i32),
        nix::sys::signal::Signal::SIGKILL,
    )
    .unwrap();

    let mut later = Vec::new();
    until(
        "a later reason kept beside gave_up",
        || {
            let status = flow.inspect();
            later = receipt(&status, &failed)
                .get("laterReasons")
                .and_then(Json::as_array)
                .into_iter()
                .flatten()
                .filter_map(|entry| field(entry, "reason").map(str::to_string))
                .collect();
            !later.is_empty()
        },
        || format!("{}\n{}", flow.inspect().compact(), read(&host.stderr)),
    );
    let status = flow.inspect();
    assert_eq!(
        field(&status, "recoveryReason"),
        Some("gave_up"),
        "{}",
        status.compact()
    );
    Unresolvable {
        failed,
        failing_plan,
        later,
        _occupied: occupied,
    }
}

/// Row 25 on `target`.
fn operator_recover_returns_the_host_to_accepting_plans(target: &'static str) {
    let flow = Flow::new("recover", target);
    let mut host = Host::start(&flow, "host");
    let state = unresolvable(&flow, &host);
    let failed = state.failed.as_str();
    assert!(
        state.later.iter().any(|reason| reason == ARTIFACT_DIFFERS),
        "the restart's artifact differs: {:?}",
        state.later
    );

    let corrective = flow.prepare("corrective");
    let (code, stderr) = flow.apply("apply-corrective", &corrective);
    assert_eq!(code, Some(2), "{stderr}");
    assert!(
        stderr.contains("host has an active or unresolved operation"),
        "{stderr}"
    );
    let (code, stderr) = flow.apply("retry-failed", &state.failing_plan);
    assert_eq!(code, Some(1), "{stderr}");
    assert!(
        stderr.contains("operation requires recovery; inspect its durable status"),
        "{stderr}"
    );

    let later = receipt(&flow.inspect(), failed)
        .get("laterReasons")
        .cloned()
        .expect("the reasons kept beside gave_up");
    let records_before = flow.journal().len();
    let (code, stdout, stderr) = flow.recover("recover", failed);
    assert_eq!(code, Some(0), "{stdout}\n{stderr}\n{}", read(&host.stderr));
    let journal = flow.journal();
    let recovered: Vec<&Json> = journal
        .iter()
        .filter(|(kind, _)| kind == "operatorRecovered")
        .map(|(_, value)| value)
        .collect();
    assert_eq!(recovered.len(), 1, "one operatorRecovered record");
    let record = recovered[0];
    assert_eq!(field(record, "operationId"), Some(failed));
    assert_eq!(field(record, "by"), Some("ignite recover"));
    let stopped: Vec<i64> = record
        .get("stopped")
        .and_then(Json::as_array)
        .expect("the stopped pids")
        .iter()
        .filter_map(Json::as_i64)
        .collect();
    assert!(stopped.iter().all(|pid| !alive(*pid)), "{stopped:?}");
    assert_eq!(
        record.get("laterReasons"),
        Some(&later),
        "{}",
        record.compact()
    );
    assert_eq!(
        stdout.trim_end(),
        format!(
            "recovered {failed}: stopped {} instances; committed revision 1 restarted",
            stopped.len()
        )
    );
    let started = journal
        .iter()
        .position(|(kind, _)| kind == "recoverStarted")
        .expect("a recoverStarted record");
    assert!(
        started >= records_before,
        "recoverStarted follows the request"
    );
    assert_eq!(
        field(&journal[started].1, "operationId"),
        Some(failed),
        "{}",
        journal[started].1.compact()
    );
    let finished = journal
        .iter()
        .position(|(kind, _)| kind == "operatorRecovered")
        .unwrap();
    let starts: Vec<String> = journal[started..finished]
        .iter()
        .filter(|(kind, _)| kind == "revisionRecord")
        .filter_map(|(_, value)| value.get("record"))
        .filter(|record| field(record, "kind") == Some("event"))
        .filter(|record| {
            record
                .get("value")
                .and_then(|value| value.get("fields"))
                .and_then(|fields| fields.get("kind"))
                .and_then(|kind| field(kind, "variant"))
                == Some("Started")
        })
        .map(Json::compact)
        .collect();
    assert!(
        starts.is_empty(),
        "no member started while recovering: {starts:?}"
    );

    let pid = await_converged(&flow, &host, 1);
    let status = flow.inspect();
    assert_eq!(
        status.get("recovery"),
        Some(&Json::Null),
        "{}",
        status.compact()
    );
    assert_eq!(status.get("recoveryReason"), Some(&Json::Null));
    let abandoned = receipt(&status, failed);
    assert_eq!(
        field(&abandoned, "state"),
        Some("abandoned"),
        "{}",
        abandoned.compact()
    );
    assert_eq!(field(&abandoned, "reason"), Some("gave_up"));
    assert_eq!(abandoned.get("laterReasons"), Some(&later));
    assert_eq!(flow.servers("terminald"), vec![pid], "one server");
    assert!(terminal_status(flow.port).contains("ready"));

    let (code, stderr) = flow.apply("retry-abandoned", &state.failing_plan);
    assert_eq!(code, Some(2), "{stderr}");
    assert!(
        stderr.contains(&format!(
            "operation {failed} was abandoned by ignite recover; prepare a new plan"
        )),
        "{stderr}"
    );

    change_command_body(&flow);
    let next = flow.prepare("next");
    let (code, stderr) = flow.apply("apply-next", &next);
    assert_eq!(code, Some(0), "{stderr}\n{}", read(&host.stderr));
    let status = flow.inspect();
    let completed = receipt(&status, &plan_id(&next));
    assert_eq!(
        field(&completed, "state"),
        Some("completed"),
        "{}",
        completed.compact()
    );
    let pid = await_converged(&flow, &host, 2);
    assert_eq!(flow.servers("terminald"), vec![pid], "one server");
    assert!(terminal_status(flow.port).contains("replacement-ready"));
    host.stop();
}

/// Row 25 on the Rust target.
#[test]
fn rust_operator_recover_returns_the_host_to_accepting_plans() {
    operator_recover_returns_the_host_to_accepting_plans("rust");
}

/// Row 25 on the LLVM target.
#[test]
fn llvm_operator_recover_returns_the_host_to_accepting_plans() {
    operator_recover_returns_the_host_to_accepting_plans("llvm");
}

/// Row 26: `ignite recover` of an operation that requires no recovery is
/// exit 2; with a process under `<state>/revisions/` that no ownership
/// record names it is exit 1, signals nothing and journals nothing; once
/// that process ended, a second `ignite recover` completes.
#[test]
fn recover_refuses_an_unaccounted_process() {
    let flow = Flow::new("refuses", "rust");
    let mut host = Host::start(&flow, "host");
    let state = unresolvable(&flow, &host);
    let failed = state.failed.as_str();

    let committed = flow
        .inspect()
        .get("committedPlan")
        .and_then(Json::as_str)
        .expect("a committed plan")
        .to_string();
    let (code, stdout, stderr) = flow.recover("recover-committed", &committed);
    assert_eq!(code, Some(2), "{stdout}\n{stderr}");
    assert!(
        stderr.contains(&format!("operation {committed} requires no recovery")),
        "{stderr}"
    );

    let bin = flow.state.join("revisions/unaccounted/bin/1");
    fs::create_dir_all(&bin).unwrap();
    let sleep = bin.join("sleep");
    fs::copy("/bin/sleep", &sleep).unwrap();
    let deadline = Instant::now() + BUSY_RETRY;
    let mut foreign = loop {
        match Command::new(&sleep).arg("300").spawn() {
            Err(error)
                if error.kind() == std::io::ErrorKind::ExecutableFileBusy
                    && Instant::now() < deadline =>
            {
                std::thread::sleep(Duration::from_millis(5));
            }
            result => break Owned(result.expect("start the unaccounted process")),
        }
    };
    let pid = i64::from(foreign.0.id());
    let before = flow.journal().len();
    let (code, stdout, stderr) = flow.recover("recover-refused", failed);
    assert_eq!(code, Some(1), "{stdout}\n{stderr}");
    assert!(
        stderr.contains(&format!(
            "recovery refused: unaccounted process {pid} runs {}",
            sleep.display()
        )),
        "{stderr}"
    );
    assert!(
        foreign.0.try_wait().unwrap().is_none() && alive(pid),
        "the unaccounted process is not signalled"
    );
    // The failed revision's run may still journal its own records; the
    // refused recover adds none of its own.
    let added: Vec<String> = flow.journal()[before..]
        .iter()
        .map(|(kind, _)| kind.clone())
        .filter(|kind| !matches!(kind.as_str(), "revisionRecord" | "ownership" | "state"))
        .collect();
    assert!(
        added.is_empty(),
        "a refused recover journals nothing: {added:?}"
    );
    assert!(
        flow.journal()
            .iter()
            .all(|(kind, _)| !matches!(kind.as_str(), "recoverStarted" | "operatorRecovered")),
        "a refused recover journals nothing"
    );
    let status = flow.inspect();
    assert_eq!(
        field(&receipt(&status, failed), "state"),
        Some("recovery_required")
    );

    foreign.0.kill().unwrap();
    foreign.0.wait().unwrap();
    let (code, stdout, stderr) = flow.recover("recover", failed);
    assert_eq!(code, Some(0), "{stdout}\n{stderr}\n{}", read(&host.stderr));
    await_converged(&flow, &host, 1);
    host.stop();
}

// ---- slice 18 wave 3, adversary: recover with a live instance ----------------

/// `source` with a third Telnet server `extra` on `port` beside `server`,
/// one restart attempt, so that a failed restart gives it up and its run
/// spawns nothing more.
fn with_extra(source: &str, port: u16) -> String {
    let members = format!(
        r#"  resource extraIngress: TcpListener {{
    address = "127.0.0.1"
    port = {port}
    exposure = Exposure.Loopback
    backlog = 128
  }}

  component extraConsole: CommandConsole {{
    implementation = module("examples.terminal.commands")
    prompt = "terminal> "
    maximum_line = 4096 bytes
    command_schema = module("examples.terminal.command_schema")
  }}

  component extra: TelnetServer {{
    listener = extraIngress
    application = extraConsole
    executable = "terminald3"
    recovery = Restart {{
      backoff = exponential(100 ms, 1 seconds)
      maximum_attempts = 1
      interval = 5 minutes
    }}
    rollout = DrainRollout {{
      deadline = 10 seconds
      on_deadline = "disconnect_remaining"
    }}
  }}

"#
    );
    let client = "  component client: Application";
    let binding = "    Terminal.client = workstationExecutable()\n";
    assert!(source.contains(client) && source.contains(binding));
    source.replace(client, &format!("{members}{client}")).replace(
        binding,
        &format!(
            "{binding}    Terminal.extraIngress = localSocket()\n    Terminal.extra = nativeProcess(\n      includes = [Terminal.extraConsole]\n    )\n"
        ),
    )
}

/// The owned pids of `members` once revision `revision` is committed, no
/// operation is active and every one of them is ready.
fn await_members(flow: &Flow, host: &Host, revision: i64, members: &[&str]) -> Vec<i64> {
    let mut pids = Vec::new();
    until(
        "every member ready",
        || {
            let status = flow.inspect();
            pids = members
                .iter()
                .filter_map(|member| {
                    let owner = status.get("ownership")?.get(member)?;
                    (owner.get("ready").and_then(Json::as_bool) == Some(true))
                        .then(|| owner.get("pid").and_then(Json::as_i64))
                        .flatten()
                })
                .collect();
            pids.len() == members.len()
                && status.get("committedRevision").and_then(Json::as_i64) == Some(revision)
                && status.get("activeOperation") == Some(&Json::Null)
        },
        || format!("{}\n{}", flow.inspect().compact(), read(&host.stderr)),
    );
    pids
}

/// The 22.9 state built to keep `server` running: revision 1 is `server` and
/// `extra`; revision 2 adds a server on a held port, which gives up; then
/// `extra`'s executable in the running revision is removed and `extra`
/// SIGKILLed, so its one restart fails to spawn ("a required owned process
/// failed to start", a later reason no corrective resolves) and the run,
/// which spawns nothing more, keeps `server` serving.
struct Live {
    failed: String,
    server: i64,
    /// `ignite recover` while the recovery was still correctable.
    correctable: (Option<i32>, String),
    _occupied: TcpListener,
}

fn live_unresolvable(flow: &Flow, host: &Host) -> Live {
    let first = flow.prepare("first");
    let (code, stderr) = flow.apply("first-apply", &first);
    assert_eq!(code, Some(0), "{stderr}");
    let pids = await_members(flow, host, 1, &["Terminal.server", "Terminal.extra"]);
    let (server, extra) = (pids[0], pids[1]);

    let occupied = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let original = read(&flow.source);
    fs::write(
        &flow.source,
        with_auxiliary(&original, occupied.local_addr().unwrap().port()),
    )
    .unwrap();
    let failing_plan = flow.prepare("failing");
    let failed = plan_id(&failing_plan);
    let (code, stderr) = flow.apply("apply-gives-up", &failing_plan);
    assert_eq!(code, Some(1), "the second server gives up: {stderr}");
    let status = flow.inspect();
    assert_eq!(
        field(&status, "recoveryReason"),
        Some("gave_up"),
        "{}",
        status.compact()
    );
    let (code, _, stderr) = flow.recover("recover-correctable", &failed);
    let correctable = (code, stderr);
    fs::write(&flow.source, original).unwrap();

    let bin = flow.run_directory(&failed).join("bin");
    for generation in fs::read_dir(&bin).unwrap().filter_map(Result::ok) {
        let _ = fs::remove_file(generation.path().join("terminald3"));
    }
    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(extra as i32),
        nix::sys::signal::Signal::SIGKILL,
    )
    .unwrap();
    until(
        "a later reason kept beside gave_up",
        || {
            receipt(&flow.inspect(), &failed)
                .get("laterReasons")
                .and_then(Json::as_array)
                .is_some_and(|later| !later.is_empty())
        },
        || format!("{}\n{}", flow.inspect().compact(), read(&host.stderr)),
    );
    // Its run settles: the failed restart gave `extra` up.
    let journal = flow.run_directory(&failed).join("journal.jsonl");
    until(
        "the run's supervisor of `extra` noted gave_up",
        || {
            read(&journal).lines().any(|line| {
                Json::parse(line).is_ok_and(|record| {
                    record.get("machine").and_then(Json::as_str)
                        == Some("Terminal.extra.supervisor")
                        && record.get("kind").and_then(Json::as_str) == Some("effect")
                        && record.get("value").is_some_and(|value| {
                            value.get("variant").and_then(Json::as_str) == Some("Note")
                                && value.get("fields").and_then(Json::as_array).is_some_and(
                                    |fields| {
                                        fields.first().and_then(Json::as_str) == Some("gave_up")
                                    },
                                )
                        })
                })
            })
        },
        || format!("{}\n{}", read(&journal), read(&host.stderr)),
    );
    Live {
        failed,
        server,
        correctable,
        _occupied: occupied,
    }
}

/// No `Started` event of any member between the first `recoverStarted`
/// and the `operatorRecovered` record (22.0 row 25).
fn started_while_recovering(journal: &[(String, Json)]) -> Vec<String> {
    let Some(started) = journal
        .iter()
        .position(|(kind, _)| kind == "recoverStarted")
    else {
        return vec!["no recoverStarted".into()];
    };
    let Some(finished) = journal
        .iter()
        .position(|(kind, _)| kind == "operatorRecovered")
    else {
        return vec!["no operatorRecovered".into()];
    };
    journal[started..finished]
        .iter()
        .filter(|(kind, _)| kind == "revisionRecord")
        .filter_map(|(_, value)| value.get("record"))
        .filter(|record| field(record, "kind") == Some("event"))
        .filter(|record| {
            record
                .get("value")
                .and_then(|value| value.get("fields"))
                .and_then(|fields| fields.get("kind"))
                .and_then(|kind| field(kind, "variant"))
                == Some("Started")
        })
        .map(Json::compact)
        .collect()
}

/// 22.9 with a live instance. 17.5: once a later reason no corrective
/// resolves stands, "the running revision dispatches no further effect";
/// 22.9 then has `ignite recover` "drain and stop every owned and retiring
/// instance of the failed and the committed revision through their
/// pidfds". So the committed revision's `server`, healthy and adopted by
/// the failed operation's run, still serves when the operator calls
/// `ignite recover`, which ends it, names it in `stopped`, restarts
/// revision 1 and lets the next plan complete. A recover while a
/// corrective still resolves the recovery is exit 2 `requires no
/// recovery`.
fn recover_ends_a_live_instance(target: &'static str) {
    let flow = Flow::new("live", target);
    let base = read(&flow.source);
    fs::write(&flow.source, with_extra(&base, free_port())).unwrap();
    let mut host = Host::start(&flow, "host");
    let state = live_unresolvable(&flow, &host);
    let failed = state.failed.as_str();
    let (code, stderr) = &state.correctable;
    assert_eq!(
        *code,
        Some(2),
        "recover of a correctable recovery: {stderr}"
    );
    assert!(
        stderr.contains(&format!("operation {failed} requires no recovery")),
        "{stderr}"
    );

    assert!(
        alive(state.server),
        "the committed revision's server {} still serves until ignite recover; the failed run's journal ends: {:?}",
        state.server,
        read(&flow.run_directory(failed).join("journal.jsonl"))
            .lines()
            .rev()
            .take(2)
            .map(|line| line.chars().take(240).collect::<String>())
            .collect::<Vec<_>>()
    );
    assert_eq!(flow.servers("terminald"), vec![state.server], "one server");

    let (code, stdout, stderr) = flow.recover("recover", failed);
    assert_eq!(code, Some(0), "{stdout}\n{stderr}\n{}", read(&host.stderr));
    let journal = flow.journal();
    let record = journal
        .iter()
        .find(|(kind, _)| kind == "operatorRecovered")
        .map(|(_, value)| value.clone())
        .expect("an operatorRecovered record");
    let stopped: Vec<i64> = record
        .get("stopped")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
        .filter_map(Json::as_i64)
        .collect();
    assert!(
        stopped.contains(&state.server),
        "the live server {} is among the stopped {stopped:?}",
        state.server
    );
    assert!(!alive(state.server), "the live server ended");
    assert_eq!(
        stdout.trim_end(),
        format!(
            "recovered {failed}: stopped {} instances; committed revision 1 restarted",
            stopped.len()
        )
    );
    assert_eq!(started_while_recovering(&journal), Vec::<String>::new());

    let pids = await_members(&flow, &host, 1, &["Terminal.server", "Terminal.extra"]);
    assert_eq!(flow.servers("terminald"), vec![pids[0]], "one server");
    change_command_body(&flow);
    let next = flow.prepare("next");
    let (code, stderr) = flow.apply("apply-next", &next);
    assert_eq!(code, Some(0), "{stderr}\n{}", read(&host.stderr));
    assert!(terminal_status(flow.port).contains("replacement-ready"));
    host.stop();
}

#[test]
fn llvm_recover_ends_a_live_instance() {
    recover_ends_a_live_instance("llvm");
}

/// Row 25's flow with the design's probe and contracts kept, as every
/// flow here keeps them since W3's fixes: the state of 22.9, `ignite recover`, then the
/// next plan replaces the server (revision 2) and must complete with the
/// replacement serving, the probe's attempts answered again.
fn recover_then_rollout_with_the_probe(target: &'static str) {
    let flow = Flow::new("probe", target);
    let design = read(&repo().join("examples/terminal/terminal.design.ignite"));
    assert!(design.contains("component promptProbe"));
    fs::write(
        &flow.source,
        design.replace("port = 2323", &format!("port = {}", flow.port)),
    )
    .unwrap();
    let host = Host::start(&flow, "host");
    let state = unresolvable(&flow, &host);
    let failed = state.failed.as_str();
    let (code, stdout, stderr) = flow.recover("recover", failed);
    assert_eq!(code, Some(0), "{stdout}\n{stderr}\n{}", read(&host.stderr));
    await_converged(&flow, &host, 1);

    change_command_body(&flow);
    let next = flow.prepare("next");
    let started = Instant::now();
    let (code, stderr) = flow.apply("apply-next", &next);
    let took = started.elapsed();
    let journals: Vec<String> = fs::read_dir(flow.state.join("revisions"))
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| {
            let text = read(&entry.path().join("journal.jsonl"));
            format!(
                "{}: {} lines, {} probe timeouts",
                entry.file_name().to_string_lossy(),
                text.lines().count(),
                text.matches("probe timeout").count()
            )
        })
        .collect();
    let host_tail: Vec<String> = read(&host.stderr)
        .lines()
        .rev()
        .take(40)
        .map(str::to_string)
        .collect();
    assert_eq!(
        code,
        Some(0),
        "the rollout after recover with the probe ({took:?}): {stderr}\n{journals:?}\n{}\n{}",
        flow.inspect().compact(),
        host_tail.join("\n")
    );
    assert!(terminal_status(flow.port).contains("replacement-ready"));
}

#[test]
fn llvm_recover_then_rollout_with_the_probe() {
    recover_then_rollout_with_the_probe("llvm");
}

#[test]
fn rust_recover_then_rollout_with_the_probe() {
    recover_then_rollout_with_the_probe("rust");
}
