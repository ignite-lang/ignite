//! Saved-plan identity and admission (`docs/slice.md` 17.3 to 17.5, 17.7 rows
//! 3, 5, 6 and the plan side of 13) through the real CLI and host socket.
//! Most cases use a system with no executables, so planning builds nothing
//! and applying starts no workload; row 6 builds the terminal design once per
//! target.

use std::cell::Cell;
use std::fs::{self, File};
use std::io::Write;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ignite::apply::plan::{CODEGEN_OVERRIDE, InputKind, Plan};
use ignite_eval::Json;

use crate::common::apply::{BUILD as BUILD_TIMEOUT, LIVE as LIVE_TIMEOUT};

/// The deadline of this file besides `LIVE_TIMEOUT` and `BUILD_TIMEOUT`:
/// how long a `Host` dropped without `stop` gets to stop before it is
/// killed; cleanup, no assertion.
const STOP_ON_DROP: Duration = Duration::from_secs(10);

/// Two loopback listeners and a helper function; nothing to build or run.
const EDGE: &str = "package admission

import std.net.{TcpListener, Exposure}
import std.deploy.{Deployment, localSocket}

fn answer() -> Int = 1

system Edge {
  resource first: TcpListener {
    address = \"127.0.0.1\"
    port = 40001
    exposure = Exposure.Loopback
    backlog = 16
  }

  resource second: TcpListener {
    address = \"127.0.0.1\"
    port = 40002
    exposure = Exposure.Loopback
    backlog = 16
  }
}

assembly local: Deployment {
  system = Edge
  bindings = {
    Edge.first = localSocket()
    Edge.second = localSocket()
  }
}
";

/// `EDGE` with its system members reordered, reformatted and commented.
const EDGE_REORDERED: &str = "package admission

import std.net.{TcpListener, Exposure}
import std.deploy.{Deployment, localSocket}

// Declaration order and layout are not operational.
fn answer() -> Int =
  1

system Edge {
  resource second: TcpListener {
    backlog = 16
    address = \"127.0.0.1\"
    exposure = Exposure.Loopback
    port = 40002
  }


  resource first: TcpListener { address = \"127.0.0.1\"
    port = 40001
    exposure = Exposure.Loopback
    backlog = 16 }
}

assembly local: Deployment {
  system = Edge
  bindings = {
    Edge.second = localSocket()
    Edge.first = localSocket()
  }
}
";

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

/// One scratch root: sources, states and every CLI output.
struct Case {
    root: PathBuf,
    counter: Cell<u64>,
}

impl Case {
    fn new(name: &str) -> Case {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "apply-admission-{name}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        Case {
            root,
            counter: Cell::new(0),
        }
    }

    /// A fresh state whose `build` is shared with the other apply tests (O41).
    fn state(&self, name: &str) -> PathBuf {
        let state = self.root.join(name);
        fs::create_dir_all(&state).unwrap();
        crate::common::apply::shared_build(&state);
        state
    }

    fn source(&self, name: &str, text: &str) -> PathBuf {
        let directory = self.root.join(name);
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("edge.ignite");
        fs::write(&path, text).unwrap();
        path
    }

    /// Run the CLI; returns stdout and stderr after checking the exit code.
    fn cli(&self, label: &str, args: &[&str], expected: i32) -> (String, String) {
        let sequence = self.counter.get();
        self.counter.set(sequence + 1);
        let stdout = self.root.join(format!("{sequence}-{label}.stdout"));
        let stderr = self.root.join(format!("{sequence}-{label}.stderr"));
        let mut child = Command::new(env!("CARGO_BIN_EXE_ignite"))
            .args(args)
            .current_dir(repo())
            .env("CARGO_INCREMENTAL", "0")
            .env("CARGO_PROFILE_DEV_DEBUG", "line-tables-only")
            .env_remove(CODEGEN_OVERRIDE)
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
            panic!("{label} exceeded {limit:?}\n{}", read(&stderr));
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

    fn plan(&self, label: &str, source: &Path, state: &Path, extra: &[&str]) -> (PathBuf, Plan) {
        let out = self.root.join(format!("{label}.json"));
        let mut args = vec![
            "plan",
            source.to_str().unwrap(),
            "--assembly",
            "local",
            "--for-apply",
            "--state",
            state.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ];
        if !extra.contains(&"--target") {
            args.extend(["--target", "rust"]);
        }
        args.extend_from_slice(extra);
        self.cli(label, &args, 0);
        let plan = Plan::parse(&Json::parse(&read(&out)).unwrap()).expect("saved apply envelope");
        (out, plan)
    }

    fn apply(&self, label: &str, plan: &Path, state: &Path, expected: i32) -> (Json, String) {
        let (stdout, stderr) = self.cli(
            label,
            &[
                "apply",
                plan.to_str().unwrap(),
                "--state",
                state.to_str().unwrap(),
            ],
            expected,
        );
        (Json::parse(&stdout).unwrap_or(Json::Null), stderr)
    }

    fn inspect(&self, state: &Path) -> Json {
        let (stdout, _) = self.cli(
            "inspect",
            &["inspect", "--state", state.to_str().unwrap(), "--json"],
            0,
        );
        Json::parse(&stdout).unwrap()
    }
}

struct Host {
    child: Option<Child>,
    stderr: PathBuf,
}

impl Host {
    fn start(case: &Case, state: &Path, label: &str, codegen: Option<&str>) -> Host {
        let stderr = case.root.join(format!("{label}.stderr"));
        let mut command = Command::new(env!("CARGO_BIN_EXE_ignite"));
        command
            .args(["host", "--state"])
            .arg(state)
            .current_dir(repo())
            .env("CARGO_INCREMENTAL", "0")
            .env("CARGO_PROFILE_DEV_DEBUG", "line-tables-only")
            .env_remove(CODEGEN_OVERRIDE)
            .stdin(Stdio::piped())
            .stdout(File::create(case.root.join(format!("{label}.stdout"))).unwrap())
            .stderr(File::create(&stderr).unwrap());
        if let Some(codegen) = codegen {
            command.env(CODEGEN_OVERRIDE, codegen);
        }
        let mut host = Host {
            child: Some(command.spawn().expect("start foreground host")),
            stderr,
        };
        let deadline = Instant::now() + LIVE_TIMEOUT;
        loop {
            assert!(
                host.child.as_mut().unwrap().try_wait().unwrap().is_none(),
                "host exited: {}",
                read(&host.stderr)
            );
            if ignite::apply::inspect(state, None)
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

fn source_pin(plan: &Plan) -> &str {
    &plan
        .inputs
        .iter()
        .find(|input| input.path == plan.source)
        .expect("source pin")
        .digest
}

fn revision(status: &Json) -> Option<i64> {
    status.get("committedRevision").and_then(Json::as_i64)
}

fn member_kind<'a>(plan: &'a Plan, id: &str) -> &'a str {
    plan.actions
        .get("members")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
        .find(|entry| entry.get("id").and_then(Json::as_str) == Some(id))
        .and_then(|entry| entry.get("kind"))
        .and_then(Json::as_str)
        .unwrap_or_else(|| panic!("no action for {id}: {:?}", plan.actions))
}

#[test]
fn reordered_members_are_operational_noop_with_fresh_pins() {
    let case = Case::new("reordered");
    let state = case.state("state");
    let source = case.source("edge", EDGE);
    let _host = Host::start(&case, &state, "host", None);
    let (first_path, first) = case.plan("first", &source, &state, &[]);
    let (receipt, _) = case.apply("first-apply", &first_path, &state, 0);
    assert_eq!(receipt.get("revision").and_then(Json::as_i64), Some(1));

    fs::write(&source, EDGE_REORDERED).unwrap();
    let (second_path, second) = case.plan("reordered", &source, &state, &[]);
    assert_eq!(first.operational_identity, second.operational_identity);
    assert_eq!(
        first.implementation_identity,
        second.implementation_identity
    );
    assert_ne!(
        source_pin(&first),
        source_pin(&second),
        "fresh admission pin"
    );
    assert_ne!(first.id, second.id);
    assert_eq!(second.base_revision, 1);
    assert_eq!(member_kind(&second, "Edge.first"), "unchanged");
    assert_eq!(member_kind(&second, "Edge.second"), "unchanged");
    let (noop, _) = case.apply("reordered-apply", &second_path, &state, 0);
    assert_eq!(noop.get("noop").and_then(Json::as_bool), Some(true));
    assert_eq!(noop.get("revision").and_then(Json::as_i64), Some(1));
    let status = case.inspect(&state);
    assert_eq!(revision(&status), Some(1));
    assert_eq!(
        status.get("committedPlan").and_then(Json::as_str),
        Some(first.id.as_str())
    );
}

#[test]
fn stale_base_changed_source_wrong_state_and_version_reject_before_mutation() {
    let case = Case::new("rejections");
    let state = case.state("state");
    let other_state = case.state("other-state");
    let source = case.source("edge", EDGE);
    let moved_source = case.source("moved", &EDGE.replace("port = 40002", "port = 40003"));
    let _host = Host::start(&case, &state, "host", None);
    let _other = Host::start(&case, &other_state, "other-host", None);
    let (first_path, first) = case.plan("first", &source, &state, &[]);
    let (moved_path, moved) = case.plan("moved", &moved_source, &state, &[]);
    assert_eq!((first.base_revision, moved.base_revision), (0, 0));
    assert_eq!(member_kind(&moved, "Edge.second"), "add");

    let (receipt, _) = case.apply("first-apply", &first_path, &state, 0);
    assert_eq!(receipt.get("revision").and_then(Json::as_i64), Some(1));
    let (_, stale) = case.apply("stale-apply", &moved_path, &state, 2);
    assert!(stale.contains("stale base"), "{stale}");
    let (retried, _) = case.apply("retry-known", &first_path, &state, 0);
    assert_eq!(
        retried.get("operationId"),
        receipt.get("operationId"),
        "receipt lookup precedes stale-base rejection"
    );

    let (edited_path, edited) = case.plan("edited", &moved_source, &state, &[]);
    assert_eq!(edited.base_revision, 1);
    assert_eq!(member_kind(&edited, "Edge.second"), "replace");
    fs::write(
        &moved_source,
        EDGE.replace("port = 40002", "port = 40003") + "// edit\n",
    )
    .unwrap();
    let (_, changed) = case.apply("changed-source", &edited_path, &state, 2);
    assert!(changed.contains("input content changed"), "{changed}");

    let (_, foreign) = case.apply("wrong-state", &edited_path, &other_state, 2);
    assert!(foreign.contains("state"), "{foreign}");
    assert_eq!(revision(&case.inspect(&other_state)), Some(0));

    let Json::Object(mut envelope) = Json::parse(&read(&edited_path)).unwrap() else {
        panic!("envelope object")
    };
    envelope.insert(
        "formatVersion".into(),
        Json::Str("ignite-apply-plan/0.2".into()),
    );
    let versioned = case.root.join("versioned.json");
    fs::write(&versioned, Json::Object(envelope).compact()).unwrap();
    let (_, version) = case.apply("wrong-version", &versioned, &state, 2);
    assert!(version.contains("formatVersion"), "{version}");

    let status = case.inspect(&state);
    assert_eq!(revision(&status), Some(1));
    assert_eq!(
        status.get("committedPlan").and_then(Json::as_str),
        Some(first.id.as_str()),
        "rejections changed nothing"
    );
}

#[test]
fn codegen_revision_and_runtime_bytes_are_separate_identity_and_admission_inputs() {
    let case = Case::new("codegen");
    let source = case.source("edge", EDGE);
    let state = case.state("state");
    let next_state = case.state("next-state");
    let host = Host::start(&case, &state, "host", None);
    let mut next = Host::start(&case, &next_state, "next-host", Some("ignite-codegen/next"));
    let (_, current) = case.plan("current", &source, &state, &[]);
    let (next_path, under_next) = case.plan("next", &source, &next_state, &[]);
    assert_eq!(
        current.codegen_revision,
        ignite::apply::plan::CODEGEN_REVISION
    );
    assert_eq!(under_next.codegen_revision, "ignite-codegen/next");
    assert_ne!(
        current.operational_identity,
        under_next.operational_identity
    );
    assert_eq!(current.reducer_revision, under_next.reducer_revision);
    assert!(
        under_next
            .inputs
            .iter()
            .any(|input| input.kind == InputKind::Compiler),
        "compiler bytes pinned"
    );

    next.stop();
    let _restarted = Host::start(&case, &next_state, "restarted-host", None);
    let (_, refused) = case.apply("other-codegen", &next_path, &next_state, 2);
    assert!(refused.contains("codegen revision"), "{refused}");
    assert_eq!(revision(&case.inspect(&next_state)), Some(0));

    let runtimes = case.root.join("runtimes");
    fs::create_dir_all(&runtimes).unwrap();
    let (one, two) = (runtimes.join("one.a"), runtimes.join("two.a"));
    fs::write(&one, b"runtime archive one").unwrap();
    fs::write(&two, b"runtime archive two").unwrap();
    let llvm = |label: &str, runtime: &Path| {
        case.plan(
            label,
            &source,
            &state,
            &["--target", "llvm", "--runtime", runtime.to_str().unwrap()],
        )
        .1
    };
    let (first, second) = (llvm("llvm-one", &one), llvm("llvm-two", &two));
    assert_eq!(
        first.operational_identity, second.operational_identity,
        "runtime archive bytes are not operational"
    );
    let pin = |plan: &Plan| {
        plan.inputs
            .iter()
            .find(|input| input.kind == InputKind::Runtime)
            .map(|input| input.digest.clone())
            .expect("runtime pinned")
    };
    assert_ne!(pin(&first), pin(&second));
    assert_ne!(first.runtime_contract, current.runtime_contract);
    assert_ne!(first.operational_identity, current.operational_identity);
    drop(host);
}

#[test]
fn changed_codegen_revision_with_an_unchanged_model_replaces_the_hosted_server() {
    codegen_replacement("rust");
}

#[test]
fn changed_codegen_revision_with_an_unchanged_model_replaces_the_hosted_server_llvm() {
    codegen_replacement("llvm");
}

/// The owned server's PID once inspect reports exactly one ready owner.
fn ready_server(case: &Case, state: &Path) -> i64 {
    let deadline = Instant::now() + LIVE_TIMEOUT;
    loop {
        let status = case.inspect(state);
        let owners: Vec<&Json> = status
            .get("ownership")
            .and_then(Json::as_object)
            .map(|owned| owned.values().collect())
            .unwrap_or_default();
        if let [owner] = owners.as_slice()
            && owner.get("ready").and_then(Json::as_bool) == Some(true)
            && let Some(pid) = owner.get("pid").and_then(Json::as_i64)
        {
            return pid;
        }
        assert!(
            Instant::now() < deadline,
            "no ready owned server: {}",
            status.compact()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Row 6: a host whose code-generation revision changed replans the
/// terminal with an unchanged model to a replacement of the server; an
/// artifact whose bytes differ from its pin is refused, and the plan with
/// its pinned bytes replaces the running server.
fn codegen_replacement(target: &str) {
    let case = Case::new(&format!("terminal-codegen-{target}"));
    let reserved = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = reserved.local_addr().unwrap().port();
    let fixture = case.root.join("terminal");
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
    let design = read(&source);
    assert!(
        design.contains("port = 2323"),
        "terminal fixture port changed"
    );
    fs::write(
        &source,
        design.replace("port = 2323", &format!("port = {port}")),
    )
    .unwrap();
    let state = case.state("state");
    let selected = crate::common::apply::target_args(target);
    let selected: Vec<&str> = selected.iter().map(String::as_str).collect();
    let plan = |label: &str| {
        let _build = crate::common::apply::build_lock(target);
        case.plan(label, &source, &state, &selected)
    };

    let mut host = Host::start(&case, &state, "host", None);
    let (first_path, first) = plan("first");
    drop(reserved);
    case.apply("first-apply", &first_path, &state, 0);
    host.stop();

    let _next = Host::start(&case, &state, "next-host", Some("ignite-codegen/next"));
    let resumed = ready_server(&case, &state);
    let (next_path, next) = plan("next");
    assert_eq!(
        ignite::apply::plan::semantic_model(&first.model),
        ignite::apply::plan::semantic_model(&next.model)
    );
    assert_ne!(first.operational_identity, next.operational_identity);
    assert_eq!(member_kind(&next, "Terminal.server"), "replace");
    assert_eq!(
        member_kind(&next, "Terminal.client"),
        "unchanged",
        "the workstation client is external and not built by the host"
    );
    assert!(
        next.actions
            .get("startOrder")
            .and_then(Json::as_array)
            .is_some_and(|order| order.contains(&Json::Str("Terminal.server".into())))
    );

    let artifact = &next.artifacts["Terminal.server"].path;
    let pinned = fs::read(artifact).unwrap();
    let mut permissions = fs::metadata(artifact).unwrap().permissions();
    let readonly = permissions.readonly();
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(false);
    fs::set_permissions(artifact, permissions.clone()).unwrap();
    let mut bytes = pinned.clone();
    bytes.push(0);
    fs::write(artifact, bytes).unwrap();
    let (_, tampered) = case.apply("tampered-artifact", &next_path, &state, 2);
    assert!(tampered.contains("input content changed"), "{tampered}");
    assert_eq!(revision(&case.inspect(&state)), Some(1));
    assert_eq!(
        ready_server(&case, &state),
        resumed,
        "the refusal changed nothing"
    );

    fs::write(artifact, &pinned).unwrap();
    permissions.set_readonly(readonly);
    fs::set_permissions(artifact, permissions).unwrap();
    let (receipt, _) = case.apply("replace", &next_path, &state, 0);
    assert_eq!(receipt.get("revision").and_then(Json::as_i64), Some(2));
    assert_eq!(receipt.get("noop").and_then(Json::as_bool), Some(false));
    assert_ne!(
        ready_server(&case, &state),
        resumed,
        "the server was replaced"
    );
}
