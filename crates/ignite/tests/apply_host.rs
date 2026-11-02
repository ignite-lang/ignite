//! The local host's edges without a compiled workload (`docs/slice.md` 17.1,
//! 17.2): control frames, the state lock, the control socket, the state
//! identity, inspect and the exit codes of `apply`. Every host here runs on
//! an empty state or on a plan with no hosted process, so no test builds a
//! program.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ignite_eval::Json;

const PROTOCOL: &str = "ignite-host-control/0.1";
const MAX_FRAME: u32 = 16 * 1024 * 1024;
const LIVE_TIMEOUT: Duration = Duration::from_secs(15);

/// A program whose assembly hosts no process: preparing it builds nothing,
/// applying it commits a revision with zero hosted machines.
const IDLE: &str = r#"package apply_idle

import std.deploy.{Deployment, workstationExecutable}

type Note {
  text: Text
}

system S {
  component note: Note { text = "idle" }
}

assembly local: Deployment {
  system = S
  bindings = {
    S.note = workstationExecutable()
  }
}
"#;

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

/// One test's directory: `state` is the host state, `idle.ignite` the
/// program, everything else is CLI output.
struct Case {
    root: PathBuf,
    state: PathBuf,
    counter: AtomicU64,
}

impl Case {
    fn new(label: &str) -> Case {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("apply-host-{label}-{}-{stamp}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("idle.ignite"), IDLE).unwrap();
        Case {
            state: root.join("state"),
            root,
            counter: AtomicU64::new(0),
        }
    }

    /// Run the CLI, require `expected` as its exit code, return stdout and
    /// stderr.
    fn cli(&self, label: &str, args: &[&str], expected: i32) -> (String, String) {
        let sequence = self.counter.fetch_add(1, Ordering::Relaxed);
        let stdout = self.root.join(format!("{sequence}-{label}.stdout"));
        let stderr = self.root.join(format!("{sequence}-{label}.stderr"));
        let mut child = Command::new(env!("CARGO_BIN_EXE_ignite"))
            .args(args)
            .current_dir(repo())
            .stdin(Stdio::null())
            .stdout(File::create(&stdout).unwrap())
            .stderr(File::create(&stderr).unwrap())
            .spawn()
            .expect("start CLI");
        let status = match wait(&mut child, LIVE_TIMEOUT) {
            Some(status) => status,
            None => {
                let _ = child.kill();
                let _ = child.wait();
                panic!(
                    "{label} exceeded {LIVE_TIMEOUT:?}\nstdout: {}\nstderr: {}",
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
        (read(&stdout), read(&stderr))
    }

    fn state_arg(&self) -> &str {
        self.state.to_str().unwrap()
    }

    /// `plan --for-apply` of the idle program against the running host.
    fn prepare(&self, label: &str) -> PathBuf {
        let out = self.root.join(format!("{label}.json"));
        let source = self.root.join("idle.ignite");
        self.cli(
            label,
            &[
                "plan",
                source.to_str().unwrap(),
                "--assembly",
                "local",
                "--for-apply",
                "--state",
                self.state_arg(),
                "--out",
                out.to_str().unwrap(),
            ],
            0,
        );
        out
    }

    fn apply(&self, label: &str, plan: &Path, expected: i32) -> (String, String) {
        self.cli(
            label,
            &["apply", plan.to_str().unwrap(), "--state", self.state_arg()],
            expected,
        )
    }

    fn inspect(&self, label: &str) -> Json {
        let (stdout, _) = self.cli(
            label,
            &["inspect", "--state", self.state_arg(), "--json"],
            0,
        );
        Json::parse(&stdout).unwrap_or_else(|reason| panic!("{label}: {reason}\n{stdout}"))
    }
}

struct Host {
    child: Option<Child>,
    stderr: PathBuf,
}

impl Host {
    fn start(case: &Case, label: &str) -> Host {
        let stderr = case.root.join(format!("{label}.stderr"));
        let child = Command::new(env!("CARGO_BIN_EXE_ignite"))
            .args(["host", "--state", case.state_arg()])
            .current_dir(repo())
            .stdin(Stdio::piped())
            .stdout(File::create(case.root.join(format!("{label}.stdout"))).unwrap())
            .stderr(File::create(&stderr).unwrap())
            .spawn()
            .expect("start host");
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
            if ignite::apply::inspect(&case.state, None)
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

    fn pid(&self) -> i64 {
        i64::from(self.child.as_ref().unwrap().id())
    }

    fn stop(&mut self) {
        let mut child = self.child.take().expect("host is running");
        let stdin = child.stdin.as_mut().expect("host control stdin");
        writeln!(stdin, "stop").unwrap();
        stdin.flush().unwrap();
        let status = wait(&mut child, LIVE_TIMEOUT);
        assert!(
            status.is_some_and(|status| status.success()),
            "host stop failed: {status:?}\n{}",
            read(&self.stderr)
        );
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// The socket's address through an open state directory, which stays under
/// the Unix socket path limit however long the state path is.
fn socket_address(state: &Path) -> (File, PathBuf) {
    let directory = File::open(state).unwrap();
    let address = PathBuf::from(format!(
        "/proc/self/fd/{}/control.sock",
        directory.as_raw_fd()
    ));
    (directory, address)
}

fn frame(body: &[u8]) -> Vec<u8> {
    let mut bytes = (body.len() as u32).to_be_bytes().to_vec();
    bytes.extend_from_slice(body);
    bytes
}

fn read_reply(stream: &mut UnixStream) -> Json {
    let mut size = [0; 4];
    stream.read_exact(&mut size).expect("reply length prefix");
    let mut body = vec![0; u32::from_be_bytes(size) as usize];
    stream.read_exact(&mut body).expect("reply body");
    Json::parse(std::str::from_utf8(&body).expect("UTF-8 reply")).expect("JSON reply")
}

/// Send raw bytes to the host's control socket and read its one reply.
fn raw(state: &Path, bytes: &[u8]) -> Json {
    let (_directory, address) = socket_address(state);
    let mut stream = UnixStream::connect(address).expect("connect control socket");
    stream.set_read_timeout(Some(LIVE_TIMEOUT)).unwrap();
    stream.write_all(bytes).expect("write raw frame");
    read_reply(&mut stream)
}

fn status_of(reply: &Json) -> (i64, &str, &str) {
    (
        reply.get("status").and_then(Json::as_i64).unwrap_or(-1),
        reply.get("requestId").and_then(Json::as_str).unwrap_or("?"),
        reply.get("version").and_then(Json::as_str).unwrap_or("?"),
    )
}

fn envelope(version: &str, id: &str) -> String {
    format!(
        r#"{{"version":"{version}","requestId":"{id}","stateIdentity":"","operation":"inspect","payload":null}}"#
    )
}

#[test]
fn malformed_oversized_and_wrong_version_frames_reject_with_status_2() {
    let case = Case::new("frames");
    let _host = Host::start(&case, "host");
    let journal = case.state.join("host-journal.jsonl");
    let journal_before = read(&journal);

    let not_json = raw(&case.state, &frame(b"{not json"));
    assert_eq!(status_of(&not_json), (2, "", PROTOCOL), "{not_json:?}");

    let not_utf8 = raw(&case.state, &frame(&[0xff, 0xfe, 0x7b]));
    assert_eq!(status_of(&not_utf8), (2, "", PROTOCOL), "{not_utf8:?}");

    let not_object = raw(&case.state, &frame(b"[1,2]"));
    assert_eq!(status_of(&not_object), (2, "", PROTOCOL), "{not_object:?}");

    let no_id = raw(
        &case.state,
        &frame(format!(r#"{{"version":"{PROTOCOL}","operation":"inspect"}}"#).as_bytes()),
    );
    assert_eq!(status_of(&no_id), (2, "", PROTOCOL), "{no_id:?}");

    let wrong_version = raw(
        &case.state,
        &frame(envelope("ignite-host-control/9.9", "r-version").as_bytes()),
    );
    assert_eq!(
        status_of(&wrong_version),
        (2, "r-version", PROTOCOL),
        "{wrong_version:?}"
    );
    assert!(
        wrong_version
            .get("reason")
            .and_then(Json::as_str)
            .is_some_and(|reason| reason.contains("version")),
        "{wrong_version:?}"
    );

    let no_operation = raw(
        &case.state,
        &frame(format!(r#"{{"version":"{PROTOCOL}","requestId":"r-op"}}"#).as_bytes()),
    );
    assert_eq!(
        status_of(&no_operation),
        (2, "r-op", PROTOCOL),
        "{no_operation:?}"
    );

    let oversized = raw(&case.state, &(MAX_FRAME + 1).to_be_bytes());
    assert_eq!(status_of(&oversized), (2, "", PROTOCOL), "{oversized:?}");
    assert!(
        oversized
            .get("reason")
            .and_then(Json::as_str)
            .is_some_and(|reason| reason.contains("16 MiB")),
        "{oversized:?}"
    );

    let good = raw(&case.state, &frame(envelope(PROTOCOL, "r-good").as_bytes()));
    assert_eq!(status_of(&good), (0, "r-good", PROTOCOL), "{good:?}");
    assert_eq!(
        read(&journal),
        journal_before,
        "a rejected frame reached admission"
    );
}

/// A scripted host on `state`'s socket: it answers one request with the
/// reply `script` builds from the request id.
fn fake_host(
    state: &Path,
    script: impl FnOnce(&str) -> Json + Send + 'static,
) -> std::thread::JoinHandle<()> {
    let (_directory, address) = socket_address(state);
    let _ = fs::remove_file(state.join("control.sock"));
    let listener = UnixListener::bind(address).expect("bind fake host");
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept client");
        let request = read_reply(&mut stream);
        let id = request
            .get("requestId")
            .and_then(Json::as_str)
            .unwrap_or("")
            .to_string();
        let reply = script(&id).compact();
        stream.write_all(&frame(reply.as_bytes())).unwrap();
    })
}

fn reply(id: &str, status: i64, reason: &str, payload: &str) -> Json {
    Json::parse(&format!(
        r#"{{"version":"{PROTOCOL}","requestId":"{id}","status":{status},"reason":"{reason}","payload":{payload}}}"#
    ))
    .unwrap()
}

#[test]
fn apply_maps_host_replies_to_exit_codes_1_2_and_3() {
    let case = Case::new("exits");
    let mut host = Host::start(&case, "host");
    let plan = case.prepare("prepare");
    host.stop();

    let served = fake_host(&case.state, |id| reply(id, 1, "workload gave up", "null"));
    let (_, stderr) = case.apply("failed", &plan, 1);
    assert!(stderr.contains("workload gave up"), "{stderr}");
    served.join().unwrap();

    let served = fake_host(&case.state, |_| {
        reply("", 2, "malformed control frame", "null")
    });
    let (_, stderr) = case.apply("rejected-without-id", &plan, 2);
    assert!(stderr.contains("malformed control frame"), "{stderr}");
    assert!(!stderr.contains("mismatch"), "{stderr}");
    served.join().unwrap();

    let served = fake_host(&case.state, |id| {
        reply(
            id,
            0,
            "",
            r#"{"operationId":"x","state":"recovery_required","reason":"outcome unknown"}"#,
        )
    });
    let (_, stderr) = case.apply("recovery-required", &plan, 1);
    assert!(stderr.contains("recovery_required"), "{stderr}");
    served.join().unwrap();

    let served = fake_host(&case.state, |_| reply("another", 0, "", "null"));
    let (_, stderr) = case.apply("foreign-reply", &plan, 3);
    assert!(stderr.contains("request id"), "{stderr}");
    served.join().unwrap();

    let (_, stderr) = case.apply("no-host", &plan, 3);
    assert!(!stderr.is_empty());
}

#[test]
fn second_host_and_run_on_a_host_state_are_refused_before_any_change() {
    let case = Case::new("exclusive");
    let host = Host::start(&case, "host");
    let socket = fs::metadata(case.state.join("control.sock")).unwrap().ino();
    let identity = read(&case.state.join("identity"));
    let journal = read(&case.state.join("host-journal.jsonl"));

    let (_, stderr) = case.cli("second-host", &["host", "--state", case.state_arg()], 2);
    assert!(stderr.contains("held by another"), "{stderr}");

    let fixture = repo().join("crates/ignite/tests/fixtures/run_gave_up.ignite");
    let (_, stderr) = case.cli(
        "run-on-host-state",
        &[
            "run",
            fixture.to_str().unwrap(),
            "--state",
            case.state_arg(),
        ],
        2,
    );
    assert!(stderr.contains("E126"), "{stderr}");

    for name in ["journal.jsonl", "bin", "build"] {
        assert!(
            !case.state.join(name).exists(),
            "refused run created {name}"
        );
    }
    assert_eq!(
        fs::metadata(case.state.join("control.sock")).unwrap().ino(),
        socket,
        "a refused process replaced the live socket"
    );
    assert_eq!(read(&case.state.join("identity")), identity);
    assert_eq!(read(&case.state.join("host-journal.jsonl")), journal);
    let status = case.inspect("inspect");
    assert_eq!(status.get("online").and_then(Json::as_bool), Some(true));
    assert_eq!(
        status
            .get("host")
            .and_then(|host| host.get("pid"))
            .and_then(Json::as_i64),
        Some(host.pid())
    );
}

#[test]
fn run_on_a_stopped_host_state_is_refused_before_any_build() {
    let case = Case::new("run-offline");
    let mut host = Host::start(&case, "host");
    host.stop();
    let fixture = repo().join("crates/ignite/tests/fixtures/run_gave_up.ignite");
    let (_, stderr) = case.cli(
        "run-on-stopped-host-state",
        &[
            "run",
            fixture.to_str().unwrap(),
            "--state",
            case.state_arg(),
        ],
        2,
    );
    assert!(
        stderr.contains("E126") && stderr.contains("ignite host"),
        "{stderr}"
    );
    for name in ["journal.jsonl", "bin", "build"] {
        assert!(
            !case.state.join(name).exists(),
            "refused run created {name}"
        );
    }
}

#[test]
fn control_socket_and_state_are_owner_only_and_a_stale_socket_is_replaced() {
    let case = Case::new("socket");
    fs::create_dir_all(&case.state).unwrap();
    fs::set_permissions(&case.state, fs::Permissions::from_mode(0o755)).unwrap();
    {
        let (_directory, address) = socket_address(&case.state);
        drop(UnixListener::bind(address).unwrap());
    }
    assert!(
        fs::metadata(case.state.join("control.sock"))
            .unwrap()
            .file_type()
            .is_socket(),
        "stale socket left behind"
    );
    let mut host = Host::start(&case, "host");
    let state_mode = fs::metadata(&case.state).unwrap().permissions().mode() & 0o777;
    assert_eq!(state_mode, 0o700, "state mode {state_mode:o}");
    let socket = fs::metadata(case.state.join("control.sock")).unwrap();
    assert!(socket.file_type().is_socket());
    assert_eq!(socket.permissions().mode() & 0o777, 0o600);
    assert_eq!(socket.uid(), fs::metadata(&case.root).unwrap().uid());
    let leftovers: Vec<_> = fs::read_dir(&case.state)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("control") && name != "control.sock")
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
    host.stop();
    assert!(!case.state.join("control.sock").exists());
}

fn identity_files(state: &Path) -> Vec<String> {
    let mut names: Vec<_> = fs::read_dir(state)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("identity"))
        .collect();
    names.sort();
    names
}

#[test]
fn state_identity_is_written_once_atomically_and_a_corrupt_one_is_refused() {
    let case = Case::new("identity");
    let mut host = Host::start(&case, "host");
    let identity = read(&case.state.join("identity"));
    assert!(
        identity.len() >= 32
            && identity
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b":-_".contains(&byte)),
        "{identity:?}"
    );
    assert_eq!(identity_files(&case.state), ["identity"]);
    let status = case.inspect("inspect");
    assert_eq!(
        status.get("stateIdentity").and_then(Json::as_str),
        Some(identity.as_str())
    );
    host.stop();
    let mut host = Host::start(&case, "restart");
    assert_eq!(read(&case.state.join("identity")), identity);
    host.stop();

    let corrupt = Case::new("identity-corrupt");
    fs::create_dir_all(&corrupt.state).unwrap();
    fs::write(corrupt.state.join("identity"), "").unwrap();
    let (_, stderr) = corrupt.cli("corrupt", &["host", "--state", corrupt.state_arg()], 3);
    assert!(stderr.contains("identity"), "{stderr}");
    assert_eq!(read(&corrupt.state.join("identity")), "");
    assert!(!corrupt.state.join("control.sock").exists());
}

#[test]
fn a_plan_missing_from_the_host_is_rejected_with_exit_2() {
    let case = Case::new("missing-plan");
    let _host = Host::start(&case, "host");
    let plan = case.prepare("prepare");
    let id = Json::parse(&read(&plan))
        .unwrap()
        .get("id")
        .and_then(Json::as_str)
        .unwrap()
        .to_string();
    fs::remove_file(case.state.join("plans").join(format!("{id}.json"))).unwrap();
    let (_, stderr) = case.apply("apply", &plan, 2);
    assert!(stderr.contains("not prepared by this host"), "{stderr}");
    let status = case.inspect("inspect");
    assert_eq!(
        status.get("committedRevision").and_then(Json::as_i64),
        Some(0)
    );
    assert_eq!(
        status
            .get("operations")
            .and_then(Json::as_object)
            .map(|ops| ops.len()),
        Some(0)
    );
}

fn field<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter().fold(value, |value, key| {
        value
            .get(key)
            .unwrap_or_else(|| panic!("missing {key} of {path:?} in {value:?}"))
    })
}

#[test]
fn inspect_reports_identity_revisions_operation_progress_readiness_and_next_action() {
    let case = Case::new("inspect");
    let mut host = Host::start(&case, "host");
    let identity = read(&case.state.join("identity"));

    let empty = case.inspect("empty");
    assert_eq!(field(&empty, &["online"]), &Json::Bool(true));
    assert_eq!(
        field(&empty, &["stateIdentity"]).as_str(),
        Some(identity.as_str())
    );
    assert_eq!(field(&empty, &["host", "pid"]).as_i64(), Some(host.pid()));
    assert!(field(&empty, &["host", "startTime"]).as_str().is_some());
    assert_eq!(field(&empty, &["committedRevision"]).as_i64(), Some(0));
    assert_eq!(field(&empty, &["desiredRevision"]).as_i64(), Some(0));
    assert_eq!(field(&empty, &["currentOperation"]), &Json::Null);
    assert_eq!(field(&empty, &["actionProgress"]), &Json::Null);
    assert_eq!(
        field(&empty, &["readiness", "converged"]),
        &Json::Bool(true)
    );
    assert_eq!(field(&empty, &["recoveryReason"]), &Json::Null);
    assert_eq!(field(&empty, &["nextAction"]).as_str(), Some("none"));

    let plan = case.prepare("prepare");
    let (stdout, _) = case.apply("apply", &plan, 0);
    let receipt = Json::parse(&stdout).unwrap();
    let operation = field(&receipt, &["operationId"])
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(field(&receipt, &["state"]).as_str(), Some("completed"));

    let applied = case.inspect("applied");
    assert_eq!(field(&applied, &["committedRevision"]).as_i64(), Some(1));
    assert_eq!(field(&applied, &["desiredRevision"]).as_i64(), Some(1));
    assert_eq!(
        field(&applied, &["currentOperation", "operationId"]).as_str(),
        Some(operation.as_str())
    );
    assert_eq!(
        field(&applied, &["currentOperation", "state"]).as_str(),
        Some("completed")
    );
    assert_eq!(
        field(&applied, &["actionProgress", "total"]).as_i64(),
        Some(1)
    );
    assert_eq!(
        field(&applied, &["actionProgress", "done"]).as_i64(),
        Some(1)
    );
    let members = field(&applied, &["actionProgress", "members"])
        .as_array()
        .unwrap();
    assert_eq!(field(&members[0], &["member"]).as_str(), Some("S.note"));
    assert_eq!(field(&members[0], &["action"]).as_str(), Some("add"));
    assert_eq!(field(&members[0], &["progress"]).as_str(), Some("done"));
    assert!(field(&applied, &["ownership"]).as_object().is_some());
    assert_eq!(
        field(&applied, &["readiness", "converged"]),
        &Json::Bool(true)
    );
    assert_eq!(field(&applied, &["nextAction"]).as_str(), Some("none"));

    let (stdout, _) = case.cli(
        "inspect-operation",
        &[
            "inspect",
            "--state",
            case.state_arg(),
            "--operation",
            &operation,
            "--json",
        ],
        0,
    );
    let one = Json::parse(&stdout).unwrap();
    assert_eq!(
        field(&one, &["operation", "state"]).as_str(),
        Some("completed")
    );
    case.cli(
        "inspect-unknown-operation",
        &[
            "inspect",
            "--state",
            case.state_arg(),
            "--operation",
            "sha256:unknown",
        ],
        2,
    );

    host.stop();
    let offline = case.inspect("offline");
    assert_eq!(field(&offline, &["online"]), &Json::Bool(false));
    assert_eq!(field(&offline, &["host"]), &Json::Null);
    assert_eq!(
        field(&offline, &["health"]).as_str(),
        Some("unknown: host offline")
    );
    assert_eq!(
        field(&offline, &["readiness"]).as_str(),
        Some("unknown: host offline")
    );
    assert_eq!(field(&offline, &["committedRevision"]).as_i64(), Some(1));
    assert_eq!(
        field(&offline, &["actionProgress", "done"]).as_i64(),
        Some(1)
    );
    assert!(
        field(&offline, &["nextAction"])
            .as_str()
            .is_some_and(|next| next.contains("ignite host")),
        "{offline:?}"
    );
    for owner in field(&offline, &["ownership"])
        .as_object()
        .unwrap()
        .values()
    {
        assert!(owner.get("ready").is_none(), "{owner:?}");
    }
}
