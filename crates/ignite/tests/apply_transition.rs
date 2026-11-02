//! The transition driver of local apply (`docs/slice.md` 17.4, 17.7 rows 1,
//! 4, 7, 8, 9): the host steps the `std.control.Transition` reducer over a
//! plan's member actions and executes what it returns against the running
//! revision. Every revision here is a variant of the terminal design that
//! keeps its imports and fns, so the implementation identity stays equal and
//! only the system's members change.

use std::fs::{self, File};
use std::io::Write;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ignite_eval::Json;

const LIVE_TIMEOUT: Duration = Duration::from_secs(20);
const BUILD_TIMEOUT: Duration = Duration::from_secs(180);
static BUILDS: Mutex<()> = Mutex::new(());

const IMPORTS: &str = r#"package examples.terminal

import std.net.{TcpListener, Exposure}
import std.protocol.telnet.{TelnetServer, TelnetProfile, Telnet}
import std.console.CommandConsole
import std.cli.{Application, Command, Argument}
import std.contract.{Availability, ThresholdCompliance, SLA}
import std.policy.{Restart, DrainRollout}
import std.protocol.telnet.TelnetPrompt
import std.deploy.{Deployment, localSocket, nativeProcess, workstationExecutable, scheduledProbeHost}
"#;

/// What one revision of the design declares.
#[derive(Clone, Copy)]
struct Revision {
    port: u16,
    server: bool,
    probe: bool,
    escape: &'static str,
    attempts: i64,
}

impl Revision {
    fn source(self) -> String {
        let mut system = String::from("system Terminal {\n");
        let mut bindings = String::new();
        if self.server {
            system.push_str(&format!(
                r#"  resource ingress: TcpListener {{
    address = "127.0.0.1"
    port = {port}
    exposure = Exposure.Loopback
    backlog = 128
  }}

  component console: CommandConsole {{
    implementation = module("examples.terminal.commands")
    prompt = "terminal> "
    maximum_line = 4096 bytes
    command_schema = module("examples.terminal.command_schema")
  }}

  component server: TelnetServer {{
    listener = ingress
    application = console
    executable = "terminald"
    protocol = TelnetProfile {{
      baseline = "nvt"
      negotiation = "q_method"
      unsupported_options = "refuse"
      character_encoding = "nvt_ascii"
    }}
    sessions = {{
      maximum = 1000
      idle_timeout = 15 minutes
      input_buffer = 16 KiB
      output_buffer = 64 KiB
      slow_reader_deadline = 30 seconds
    }}
    recovery = Restart {{
      backoff = exponential(100 ms, 1 seconds)
      maximum_attempts = {attempts}
      interval = 5 minutes
    }}
    rollout = DrainRollout {{
      deadline = 10 seconds
      on_deadline = "disconnect_remaining"
    }}
  }}

"#,
                port = self.port,
                attempts = self.attempts,
            ));
            bindings.push_str(
                "    Terminal.ingress = localSocket()\n    Terminal.server = nativeProcess(\n      includes = [Terminal.console]\n    )\n",
            );
        }
        system.push_str(&format!(
            r#"  component client: Application {{
    executable = "tn"
    commands = {{
      connect = Command {{
        arguments = [Argument {{
          name = "endpoint"
          kind = endpoint(Telnet)
        }}]
        handler = module("std.protocol.telnet")
        terminal = "interactive"
        escape = "{escape}"
      }}
    }}
  }}
"#,
            escape = self.escape
        ));
        bindings.push_str("    Terminal.client = workstationExecutable()\n");
        if self.probe {
            system.push_str(
                r#"
  component promptProbe: TelnetPrompt {
    target = server
    interval = 10 seconds
    timeout = 2 seconds
    expected_prompt = "terminal> "
    close_after_probe = true
  }

  contract reachable: Availability {
    signal = promptProbe.attempts
    good = "prompt_received_before_timeout"
    target = 99.9%
    window = rolling(30 days)
    missing_data = "insufficient_evidence"
  }
"#,
            );
            bindings.push_str("    Terminal.promptProbe = scheduledProbeHost()\n");
        }
        system.push_str("}\n");
        format!(
            "{IMPORTS}\n{system}\nassembly local: Deployment {{\n  system = Terminal\n  bindings = {{\n{bindings}  }}\n}}\n"
        )
    }
}

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

fn free_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

struct Flow {
    root: PathBuf,
    state: PathBuf,
    source: PathBuf,
    counter: std::cell::Cell<u64>,
}

impl Flow {
    /// A fixture directory with the terminal's sibling modules and a state
    /// whose `build` is shared with every apply test of this binary (O41).
    fn new(label: &str) -> Flow {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("transition-{label}-{}-{stamp}", std::process::id()));
        let fixture = root.join("terminal");
        fs::create_dir_all(&fixture).unwrap();
        for name in ["commands.ignite", "command_schema.ignite"] {
            fs::copy(
                repo().join("examples/terminal").join(name),
                fixture.join(name),
            )
            .unwrap();
        }
        let state = root.join("state");
        fs::create_dir_all(&state).unwrap();
        let shared = Path::new(env!("CARGO_TARGET_TMPDIR")).join("apply-build");
        fs::create_dir_all(&shared).unwrap();
        std::os::unix::fs::symlink(&shared, state.join("build")).unwrap();
        Flow {
            source: fixture.join("terminal.design.ignite"),
            state,
            root,
            counter: std::cell::Cell::new(0),
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
        let Some(status) = wait(&mut child, limit) else {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "{label} exceeded {limit:?}\nstdout: {}\nstderr: {}",
                read(&stdout),
                read(&stderr)
            );
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

    /// Write `revision` and prepare it against the host; the plan's path
    /// and the plan.
    fn prepare(&self, label: &str, revision: Revision) -> (PathBuf, Json) {
        fs::write(&self.source, revision.source()).unwrap();
        let output = self.root.join(format!("{label}.json"));
        self.cli(
            label,
            &[
                "plan",
                self.source.to_str().unwrap(),
                "--assembly",
                "local",
                "--for-apply",
                "--state",
                self.state.to_str().unwrap(),
                "--out",
                output.to_str().unwrap(),
            ],
            0,
        );
        let plan = Json::parse(&read(&output)).unwrap();
        (output, plan)
    }

    fn apply(&self, label: &str, plan: &Path, expected: i32) -> String {
        self.cli(
            label,
            &[
                "apply",
                plan.to_str().unwrap(),
                "--state",
                self.state.to_str().unwrap(),
            ],
            expected,
        )
    }

    fn applied(&self, label: &str, plan: &Path) -> Json {
        let output = self.apply(label, plan, 0);
        Json::parse(&output).unwrap_or_else(|reason| panic!("{label}: {reason}\n{output}"))
    }

    fn inspect(&self) -> Json {
        let output = self.cli(
            "inspect",
            &["inspect", "--state", self.state.to_str().unwrap(), "--json"],
            0,
        );
        Json::parse(&output).unwrap()
    }

    /// The host journal's records of `kind`, values only, in order.
    fn host_records(&self, kind: &str) -> Vec<Json> {
        read(&self.state.join("host-journal.jsonl"))
            .lines()
            .filter_map(|line| Json::parse(line).ok())
            .filter(|record| record.get("kind").and_then(Json::as_str) == Some(kind))
            .filter_map(|record| record.get("value").cloned())
            .collect()
    }

    /// Every transition step of operation `id`: the event and the effects,
    /// as `Variant field field`.
    fn steps(&self, id: &str) -> Vec<(String, Vec<String>)> {
        self.host_records("transition")
            .into_iter()
            .filter(|step| step.get("operationId").and_then(Json::as_str) == Some(id))
            .map(|step| {
                let effects = step
                    .get("effects")
                    .and_then(Json::as_array)
                    .unwrap()
                    .iter()
                    .map(variant_text)
                    .collect();
                (variant_text(step.get("event").unwrap()), effects)
            })
            .collect()
    }

    /// The effects of operation `id` in order.
    fn effects(&self, id: &str) -> Vec<String> {
        self.steps(id)
            .into_iter()
            .flat_map(|(_, effects)| effects)
            .collect()
    }

    /// Every record of every run journal of this host.
    fn run_records(&self) -> Vec<Json> {
        let mut records = Vec::new();
        let Ok(entries) = fs::read_dir(self.state.join("revisions")) else {
            return records;
        };
        let mut directories: Vec<PathBuf> = entries.map(|entry| entry.unwrap().path()).collect();
        directories.sort();
        for directory in directories {
            records.extend(
                read(&directory.join("journal.jsonl"))
                    .lines()
                    .filter_map(|line| Json::parse(line).ok()),
            );
        }
        records
    }
}

/// `Variant field field` of an encoded enum value.
fn variant_text(value: &Json) -> String {
    let mut text = value
        .get("variant")
        .and_then(Json::as_str)
        .unwrap_or("?")
        .to_string();
    for field in value
        .get("fields")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
    {
        text.push(' ');
        text.push_str(field.as_str().unwrap_or(&field.compact()));
    }
    text
}

fn plan_id(plan: &Json) -> String {
    plan.get("id").and_then(Json::as_str).unwrap().to_string()
}

fn action<'a>(plan: &'a Json, member: &str) -> &'a Json {
    plan.get("actions")
        .and_then(|actions| actions.get("members"))
        .and_then(Json::as_array)
        .unwrap()
        .iter()
        .find(|entry| entry.get("id").and_then(Json::as_str) == Some(member))
        .unwrap_or_else(|| panic!("no action for {member}"))
}

fn kind(plan: &Json, member: &str) -> String {
    action(plan, member)
        .get("kind")
        .and_then(Json::as_str)
        .unwrap()
        .to_string()
}

fn owned_pid(status: &Json, member: &str) -> Option<i64> {
    status
        .get("ownership")
        .and_then(|owned| owned.get(member))
        .and_then(|owner| owner.get("pid"))
        .and_then(Json::as_i64)
}

fn alive(pid: i64) -> bool {
    fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|stat| {
        stat.rsplit_once(") ")
            .is_some_and(|(_, fields)| !fields.starts_with('Z'))
    })
}

fn position(effects: &[String], wanted: &str) -> usize {
    effects
        .iter()
        .position(|effect| effect == wanted)
        .unwrap_or_else(|| panic!("no {wanted} in {effects:?}"))
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
        while !ignite::apply::inspect(&flow.state, None)
            .is_ok_and(|status| status.get("online").and_then(Json::as_bool) == Some(true))
        {
            assert!(
                host.child.as_mut().unwrap().try_wait().unwrap().is_none(),
                "host exited: {}",
                read(&host.stderr)
            );
            assert!(Instant::now() < deadline, "host socket deadline");
            std::thread::sleep(Duration::from_millis(20));
        }
        host
    }

    fn stop(&mut self) {
        let mut child = self.child.take().expect("host is running");
        let stdin = child.stdin.as_mut().expect("host control stdin");
        writeln!(stdin, "stop").unwrap();
        stdin.flush().unwrap();
        let status = wait(&mut child, Duration::from_secs(30));
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
            if wait(&mut child, Duration::from_secs(10)).is_none() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

/// 17.7 rows 1 (probe and standing part), 7 and 8: a provider and an
/// external client, then a dependent probe and its contract next to the
/// running provider, then the removal of every hosted member, then the
/// provider and its dependent together.
#[test]
fn add_remove_dependent_orders_provider_first_external_untouched() {
    let _builds = BUILDS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let flow = Flow::new("dependent");
    let port = free_port();
    let provider = Revision {
        port,
        server: true,
        probe: false,
        escape: "Ctrl+]",
        attempts: 10,
    };
    let dependent = Revision {
        probe: true,
        escape: "Ctrl+^",
        ..provider
    };
    let idle = Revision {
        server: false,
        probe: false,
        ..dependent
    };
    let mut host = Host::start(&flow, "host");

    let (first, first_plan) = flow.prepare("provider", provider);
    let receipt = flow.applied("apply-provider", &first);
    assert_eq!(receipt.get("revision").and_then(Json::as_i64), Some(1));
    let pid = owned_pid(&flow.inspect(), "Terminal.server").expect("owned server");
    let first_effects = flow.effects(&plan_id(&first_plan));
    assert_eq!(
        first_effects.first().map(String::as_str),
        Some("SwitchRevision")
    );
    assert_eq!(first_effects.last().map(String::as_str), Some("Commit"));
    assert!(
        position(&first_effects, "StartMember Terminal.ingress")
            < position(&first_effects, "StartMember Terminal.server"),
        "{first_effects:?}"
    );

    let (second, second_plan) = flow.prepare("dependent", dependent);
    assert_eq!(kind(&second_plan, "Terminal.server"), "unchanged");
    assert_eq!(kind(&second_plan, "Terminal.promptProbe"), "add");
    assert_eq!(kind(&second_plan, "Terminal.client"), "detach");
    assert_eq!(
        action(&second_plan, "Terminal.client")
            .get("ownership")
            .and_then(Json::as_str),
        Some("external")
    );
    let receipt = flow.applied("apply-dependent", &second);
    assert_eq!(receipt.get("revision").and_then(Json::as_i64), Some(2));
    let status = flow.inspect();
    assert_eq!(
        owned_pid(&status, "Terminal.server"),
        Some(pid),
        "the unchanged provider keeps its process: {status:?}"
    );
    assert!(alive(pid));
    let second_id = plan_id(&second_plan);
    assert_eq!(
        flow.effects(&second_id),
        [
            "SwitchRevision",
            "StartMember Terminal.promptProbe",
            "StartMember Terminal.reachable",
            "Commit"
        ]
    );
    // Row 1: the new revision's probe emission and standing come before the
    // commit.
    let records = flow.host_records("revisionRecord");
    let committed = flow
        .host_records("transition")
        .iter()
        .position(|step| {
            step.get("operationId").and_then(Json::as_str) == Some(second_id.as_str())
                && step
                    .get("effects")
                    .and_then(Json::as_array)
                    .is_some_and(|effects| effects.iter().any(|e| variant_text(e) == "Commit"))
        })
        .expect("commit step");
    let journal: Vec<Json> = read(&flow.state.join("host-journal.jsonl"))
        .lines()
        .filter_map(|line| Json::parse(line).ok())
        .collect();
    let commit_at = journal
        .iter()
        .enumerate()
        .filter(|(_, record)| record.get("kind").and_then(Json::as_str) == Some("transition"))
        .nth(committed)
        .map(|(index, _)| index)
        .unwrap();
    let before_commit = |kind: &str, machine: &str| {
        journal[..commit_at].iter().any(|record| {
            let value = record.get("value");
            record.get("kind").and_then(Json::as_str) == Some("revisionRecord")
                && value
                    .and_then(|v| v.get("operationId"))
                    .and_then(Json::as_str)
                    == Some(second_id.as_str())
                && value.and_then(|v| v.get("record")).is_some_and(|record| {
                    record.get("kind").and_then(Json::as_str) == Some(kind)
                        && record.get("machine").and_then(Json::as_str) == Some(machine)
                })
        })
    };
    assert!(
        before_commit("signal", "Terminal.promptProbe.scheduler"),
        "{records:?}"
    );
    assert!(before_commit("standing", "Terminal.reachable.evaluator"));

    let (third, third_plan) = flow.prepare("idle", idle);
    assert_eq!(kind(&third_plan, "Terminal.server"), "remove");
    assert_eq!(kind(&third_plan, "Terminal.promptProbe"), "remove");
    let receipt = flow.applied("apply-idle", &third);
    assert_eq!(receipt.get("revision").and_then(Json::as_i64), Some(3));
    let third_id = plan_id(&third_plan);
    let steps = flow.steps(&third_id);
    let effects = flow.effects(&third_id);
    let probe_stop = position(&effects, "StopMember Terminal.promptProbe");
    let server_stop = position(&effects, "StopMember Terminal.server");
    assert!(probe_stop < server_stop, "{effects:?}");
    let stopped_probe = steps
        .iter()
        .position(|(event, _)| event == "MemberStopped Terminal.promptProbe")
        .expect("probe stopped");
    let stopping_server = steps
        .iter()
        .position(|(_, effects)| effects.iter().any(|e| e == "StopMember Terminal.server"))
        .unwrap();
    assert!(
        stopped_probe <= stopping_server,
        "the dependent stops before its provider: {steps:?}"
    );
    assert!(!alive(pid), "the removed provider's process ended");
    let status = flow.inspect();
    assert_eq!(status.get("online").and_then(Json::as_bool), Some(true));
    assert_eq!(
        status
            .get("ownership")
            .and_then(Json::as_object)
            .map(|o| o.len()),
        Some(0),
        "{status:?}"
    );
    // Row 8: the same desired state planned again is an operational no-op.
    let (again, _) = flow.prepare("idle-again", idle);
    let noop = flow.applied("apply-idle-again", &again);
    assert_eq!(noop.get("noop").and_then(Json::as_bool), Some(true));
    assert_eq!(noop.get("revision").and_then(Json::as_i64), Some(3));

    let (fourth, fourth_plan) = flow.prepare("together", dependent);
    let receipt = flow.applied("apply-together", &fourth);
    assert_eq!(receipt.get("revision").and_then(Json::as_i64), Some(4));
    let fourth_id = plan_id(&fourth_plan);
    let steps = flow.steps(&fourth_id);
    let start_server = steps
        .iter()
        .position(|(_, effects)| effects.iter().any(|e| e == "StartMember Terminal.server"))
        .unwrap();
    let server_ready = steps
        .iter()
        .position(|(event, _)| event == "MemberReady Terminal.server")
        .unwrap();
    let start_probe = steps
        .iter()
        .position(|(_, effects)| {
            effects
                .iter()
                .any(|e| e == "StartMember Terminal.promptProbe")
        })
        .unwrap();
    assert!(
        start_server < server_ready && server_ready <= start_probe,
        "the provider is ready before its dependent starts: {steps:?}"
    );

    // Row 7: the external client is never started, stopped or signalled.
    for id in [&plan_id(&first_plan), &second_id, &third_id, &fourth_id] {
        for (event, effects) in flow.steps(id) {
            assert!(!event.contains("Terminal.client"), "{event}");
            assert!(
                effects
                    .iter()
                    .all(|effect| !effect.contains("Terminal.client")),
                "{effects:?}"
            );
        }
    }
    for record in flow.run_records() {
        if record.get("kind").and_then(Json::as_str) == Some("effect") {
            assert!(
                !record.compact().contains("Terminal.client"),
                "{}",
                record.compact()
            );
        }
    }
    host.stop();
}

/// D3, 17.7 rows 8 and 9's failure half: a supervisor that gives up during a
/// transition fails the operation with `recovery_required` and reason
/// `gave_up`; a corrective apply whose base is the committed revision clears
/// it.
#[test]
fn a_machine_that_gives_up_fails_the_operation_until_a_corrective_apply() {
    let _builds = BUILDS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let flow = Flow::new("gave-up");
    let occupied = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let broken = Revision {
        port: occupied.local_addr().unwrap().port(),
        server: true,
        probe: false,
        escape: "Ctrl+]",
        attempts: 1,
    };
    let mut host = Host::start(&flow, "host");
    let (failing, failing_plan) = flow.prepare("occupied", broken);
    flow.apply("apply-occupied", &failing, 1);
    let status = flow.inspect();
    let current = status.get("currentOperation").unwrap();
    assert_eq!(
        current.get("operationId").and_then(Json::as_str),
        Some(plan_id(&failing_plan).as_str())
    );
    assert_eq!(
        current.get("state").and_then(Json::as_str),
        Some("recovery_required")
    );
    assert_eq!(
        current.get("reason").and_then(Json::as_str),
        Some("gave_up")
    );
    assert_eq!(
        status.get("recoveryReason").and_then(Json::as_str),
        Some("gave_up")
    );
    assert_eq!(
        status.get("committedRevision").and_then(Json::as_i64),
        Some(0)
    );
    assert!(
        flow.effects(&plan_id(&failing_plan))
            .last()
            .is_some_and(|effect| effect == "Abandon gave_up Terminal.server"),
        "{:?}",
        flow.effects(&plan_id(&failing_plan))
    );

    let (corrective, _) = flow.prepare(
        "corrective",
        Revision {
            port: free_port(),
            attempts: 10,
            ..broken
        },
    );
    let receipt = flow.applied("apply-corrective", &corrective);
    assert_eq!(receipt.get("revision").and_then(Json::as_i64), Some(1));
    let status = flow.inspect();
    assert_eq!(status.get("recoveryReason"), Some(&Json::Null));
    assert!(owned_pid(&status, "Terminal.server").is_some_and(alive));
    drop(occupied);
    host.stop();
}
