//! The terminal design with its probe kept, through a rollout that hands
//! the running revision's instances to a new run (`docs/slice.md` 17.4,
//! 22.9): `apply_way_out.rs` strips the probe ([`without_probe`] there)
//! because with it the handover to revision 2 stalled once, every probe
//! attempt after the handover request timing out. These cases keep the
//! probe and assert the handover happens: revision 2 gets a run directory
//! of its own and the apply ends.
//!
//! Only `plan`, `apply`, `inspect` and `host` are used, so the file runs
//! unchanged on bases without `ignite recover`.

use std::fs::{self, File};
use std::io::Write;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ignite_eval::Json;

use crate::common::apply::{BUILD as BUILD_TIMEOUT, LIVE as LIVE_TIMEOUT};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

fn tail(text: &str, lines: usize) -> String {
    let all: Vec<&str> = text.lines().collect();
    all[all.len().saturating_sub(lines)..].join("\n")
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

struct Host {
    child: Option<Child>,
    stderr: PathBuf,
}

impl Host {
    fn start(flow: &Flow) -> Host {
        let stderr = flow.root.join("host.stderr");
        let child = Command::new(env!("CARGO_BIN_EXE_ignite"))
            .args(["host", "--state"])
            .arg(&flow.state)
            .current_dir(repo())
            .env("CARGO_INCREMENTAL", "0")
            .env("CARGO_PROFILE_DEV_DEBUG", "line-tables-only")
            .stdin(Stdio::piped())
            .stdout(File::create(flow.root.join("host.stdout")).unwrap())
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
                break;
            }
            assert!(Instant::now() < deadline, "host socket");
            std::thread::sleep(Duration::from_millis(20));
        }
        host
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
            if wait(&mut child, Duration::from_secs(20)).is_none() {
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
    target: &'static str,
    counter: std::cell::Cell<u64>,
}

impl Flow {
    /// A fresh copy of the terminal design, probe and contracts kept, on a
    /// free port.
    fn new(label: &str, target: &'static str) -> Flow {
        let port = free_port();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "probe-handover-{label}-{target}-{}-{stamp}",
            std::process::id()
        ));
        let fixture = root.join("terminal");
        fs::create_dir_all(&fixture).unwrap();
        for entry in fs::read_dir(repo().join("examples/terminal")).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "ignite") {
                fs::copy(&path, fixture.join(path.file_name().unwrap())).unwrap();
            }
        }
        let source = fixture.join("terminal.design.ignite");
        let original = read(&source);
        assert!(
            original.contains("component promptProbe"),
            "the probe is kept"
        );
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
            target,
            counter: std::cell::Cell::new(0),
        }
    }

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
            return (None, read(&stdout), read(&stderr));
        };
        (status.code(), read(&stdout), read(&stderr))
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
        let (code, stdout, stderr) = self.run(label, &args);
        assert_eq!(code, Some(0), "{label}\n{stdout}\n{stderr}");
        output
    }

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

    fn inspect(&self) -> Json {
        ignite::apply::inspect(&self.state, None).expect("inspect")
    }

    /// The plan ids of the revision run directories under `revisions/`.
    fn run_directories(&self) -> Vec<String> {
        let mut found: Vec<String> = fs::read_dir(self.state.join("revisions"))
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .filter_map(|entry| {
                Json::parse(&read(&entry.path().join("context.json")))
                    .ok()?
                    .get("planId")
                    .and_then(Json::as_str)
                    .map(str::to_string)
            })
            .collect();
        found.sort();
        found
    }

    /// What a stalled handover leaves to look at: the transition steps of
    /// the host journal, the probe notes of every revision journal, the
    /// host's stderr.
    fn diagnosis(&self, host: &Host) -> String {
        let journal = read(&self.state.join("host-journal.jsonl"));
        let transitions: Vec<String> = journal
            .lines()
            .filter_map(|line| Json::parse(line).ok())
            .filter(|record| record.get("kind").and_then(Json::as_str) == Some("transition"))
            .filter_map(|record| {
                let value = record.get("value")?;
                Some(format!(
                    "{} -> {}",
                    value.get("event").map(Json::compact).unwrap_or_default(),
                    value.get("effects").map(Json::compact).unwrap_or_default()
                ))
            })
            .collect();
        let mut notes = Vec::new();
        for entry in fs::read_dir(self.state.join("revisions"))
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
        {
            let text = read(&entry.path().join("journal.jsonl"));
            let count = text.matches("probe timeout").count();
            notes.push(format!(
                "{}: {count} probe timeouts, {} lines",
                entry.file_name().to_string_lossy(),
                text.lines().count()
            ));
        }
        format!(
            "status {}\ntransitions:\n{}\nrevision journals: {notes:?}\nhost stderr (tail):\n{}",
            self.inspect().compact(),
            transitions.join("\n"),
            tail(&read(&host.stderr), 60)
        )
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

/// `source` with a second Telnet server on `port`; with `attempts` 1 and
/// the port held by the test, it exits at once and gives up.
fn with_auxiliary(source: &str, port: u16, attempts: u32) -> String {
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
      maximum_attempts = {attempts}
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

/// Revision 1 committed, with the probe; then a revision 2 that adds a
/// member. `occupied` holds revision 2's new port, so its server gives up.
fn probe_kept_rollout(target: &'static str, label: &str, gives_up: bool) {
    let flow = Flow::new(label, target);
    let host = Host::start(&flow);
    let first = flow.prepare("first");
    let (code, stderr) = flow.apply("first-apply", &first);
    assert_eq!(
        code,
        Some(0),
        "revision 1: {stderr}\n{}",
        flow.diagnosis(&host)
    );
    assert_eq!(flow.run_directories(), vec![plan_id(&first)]);

    let occupied = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = if gives_up {
        occupied.local_addr().unwrap().port()
    } else {
        free_port()
    };
    let original = read(&flow.source);
    fs::write(
        &flow.source,
        with_auxiliary(&original, port, if gives_up { 1 } else { 3 }),
    )
    .unwrap();
    let second = flow.prepare("second");
    let started = Instant::now();
    let (code, stderr) = flow.apply("second-apply", &second);
    let elapsed = started.elapsed();
    let directories = flow.run_directories();
    let mut expected = vec![plan_id(&first), plan_id(&second)];
    expected.sort();
    assert_eq!(
        directories,
        expected,
        "revision 2 got no run of its own (handover) after {elapsed:?}; apply exit {code:?}\n{stderr}\n{}",
        flow.diagnosis(&host)
    );
    let status = flow.inspect();
    if gives_up {
        assert_eq!(code, Some(1), "{stderr}\n{}", flow.diagnosis(&host));
        assert_eq!(
            status.get("recoveryReason").and_then(Json::as_str),
            Some("gave_up"),
            "{}",
            status.compact()
        );
    } else {
        assert_eq!(code, Some(0), "{stderr}\n{}", flow.diagnosis(&host));
        assert_eq!(
            status.get("committedRevision").and_then(Json::as_i64),
            Some(2),
            "{}",
            status.compact()
        );
    }
    drop(occupied);
    drop(host);
}

#[test]
fn rust_probe_kept_rollout_adding_a_member_hands_over() {
    probe_kept_rollout("rust", "adds", false);
}

#[test]
fn llvm_probe_kept_rollout_adding_a_member_hands_over() {
    probe_kept_rollout("llvm", "adds", false);
}

#[test]
fn rust_probe_kept_rollout_whose_new_member_gives_up_hands_over() {
    probe_kept_rollout("rust", "gives-up", true);
}

#[test]
fn llvm_probe_kept_rollout_whose_new_member_gives_up_hands_over() {
    probe_kept_rollout("llvm", "gives-up", true);
}
