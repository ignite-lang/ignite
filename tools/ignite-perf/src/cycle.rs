//! Mode `apply-cycle` (`docs/slice.md` 22.15, row 28 of 22.0): the edit-build-restart cycle
//! under `ignite apply`. Per source tree, target and system: a temporary working directory with
//! a copy of the example, `ignite host` running in it and revision 1 applied; then per sample one
//! response literal of the server changes, `ignite plan --for-apply` builds the artifact,
//! `ignite apply` runs until `completed`, and the replaced server is asked until it answers the
//! new text. One warm-up sample is discarded (phase `warmup`), five are measured. The terminal
//! design runs on every tree, the todo on the candidate only (D12).

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::json::{Json, obj};
use crate::perf::{Harness, R, SCHEMA, ensure, exit_code, io_err, loadavg, path_str};

/// Measured samples per tree, target and system, after [`WARMUP`] discarded ones.
pub const SAMPLES: usize = 5;
pub const WARMUP: usize = 1;
pub const TARGETS: [&str; 2] = ["rust", "llvm"];

/// Ceiling of a wait that ends at an observed event (host socket, answer, host stop).
const LIVE: Duration = Duration::from_secs(120);
/// Ceiling of one `plan --for-apply`; the first one of a state compiles the substrate.
const BUILD: Duration = Duration::from_secs(900);
/// Ceiling of one `ignite apply`: above a rollout deadline (120 s for the terminal), so an
/// apply that waits for the previous rollout's deadline is recorded, not killed.
const APPLY: Duration = Duration::from_secs(600);

/// One example the cycle applies and edits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum System {
    Terminal,
    Todo,
}

impl System {
    pub fn name(self) -> &'static str {
        match self {
            System::Terminal => "terminal",
            System::Todo => "todo",
        }
    }

    /// The example's directory in a source tree; the copy keeps the same relative path.
    pub fn directory(self) -> &'static str {
        match self {
            System::Terminal => "examples/terminal",
            System::Todo => "examples/todo",
        }
    }

    /// The file `ignite plan` reads.
    pub fn source(self) -> &'static str {
        match self {
            System::Terminal => "terminal.design.ignite",
            System::Todo => "todo.ignite",
        }
    }

    /// The file whose response literal a sample changes.
    pub fn edited(self) -> &'static str {
        match self {
            System::Terminal => "commands.ignite",
            System::Todo => "todo.ignite",
        }
    }

    /// The listener's port as the example declares it.
    pub fn port_text(self) -> &'static str {
        match self {
            System::Terminal => "port = 2323",
            System::Todo => "port = 8080",
        }
    }

    /// The text the unedited server answers.
    pub fn original(self) -> &'static str {
        match self {
            System::Terminal => "ready",
            System::Todo => "ok",
        }
    }

    /// The source of the response literal answering `text`: the terminal's `status` reply,
    /// the todo's `GET /health` body (`text` plus a newline).
    pub fn literal(self, text: &str) -> String {
        match self {
            System::Terminal => format!("Status => Reply {{ lines = [\"{text}\"] }}"),
            System::Todo => {
                format!("(\"GET\", [\"health\"]) => Response.plain(200, \"{text}\\n\")")
            }
        }
    }

    /// The text sample `number` makes the server answer.
    pub fn text(self, number: usize) -> String {
        format!("{} {number}", self.original())
    }

    pub fn unit(self, target: &str) -> String {
        format!("apply-cycle-{}-{target}", self.name())
    }
}

/// The systems a tree labelled `label` cycles: the terminal design on every tree, the todo on
/// the candidate only, because the todo cannot be applied at the slice base (D12).
pub fn systems(label: &str) -> Vec<System> {
    if label == "candidate" {
        vec![System::Terminal, System::Todo]
    } else {
        vec![System::Terminal]
    }
}

/// The units `apply-cycle` writes for a tree labelled `label`.
pub fn units(label: &str) -> Vec<String> {
    TARGETS
        .iter()
        .flat_map(|target| systems(label).into_iter().map(move |s| s.unit(target)))
        .collect()
}

/// `text` with its one occurrence of `from` replaced by `to`; none or several are refused, so
/// an example that drifted from the harness fails before anything is timed.
pub fn replace_once(text: &str, from: &str, to: &str) -> R<String> {
    let count = text.matches(from).count();
    ensure(
        count == 1,
        format!("apply-cycle: expected one `{from}`, found {count}"),
    )?;
    Ok(text.replacen(from, to, 1))
}

/// The phase of sample `number`: the first [`WARMUP`] are discarded.
pub fn phase(number: usize) -> &'static str {
    if number < WARMUP {
        "warmup"
    } else {
        "measured"
    }
}

fn median(mut values: Vec<f64>) -> Json {
    if values.is_empty() {
        return Json::Null;
    }
    values.sort_by(f64::total_cmp);
    let n = values.len();
    Json::Float(if n % 2 == 1 {
        values[n / 2]
    } else {
        (values[n / 2 - 1] + values[n / 2]) / 2.0
    })
}

/// Per tree and unit of the apply cycle: the measured samples that succeeded, the failed ones,
/// and the medians of the total and of the split (plan with its build, apply until `completed`,
/// first answer); warm-up samples are left out. `todo_llvm_below_rust` is row 28's observation
/// on the candidate, null until both todo units have a median.
pub fn summary(rows: &[Json]) -> Json {
    let mut keys: Vec<(String, String)> = Vec::new();
    for row in rows {
        let unit = row.field("unit").as_str().unwrap_or("");
        let label = row.field("label").as_str().unwrap_or("");
        let key = (label.to_string(), unit.to_string());
        if unit.starts_with("apply-cycle-")
            && row.field("phase").as_str() == Some("measured")
            && !keys.contains(&key)
        {
            keys.push(key);
        }
    }
    let mut results = Vec::new();
    for (label, unit) in &keys {
        let samples: Vec<&Json> = rows
            .iter()
            .filter(|r| {
                r.field("label").as_str() == Some(label.as_str())
                    && r.field("unit").as_str() == Some(unit.as_str())
                    && r.field("phase").as_str() == Some("measured")
            })
            .collect();
        let good: Vec<&&Json> = samples
            .iter()
            .filter(|r| !r.field("exit").truthy())
            .collect();
        let values = |key: &str| -> Json {
            median(good.iter().filter_map(|r| r.field(key).as_f64()).collect())
        };
        results.push(obj(vec![
            ("label", label.as_str().into()),
            ("unit", unit.as_str().into()),
            ("samples", good.len().into()),
            ("failed", (samples.len() - good.len()).into()),
            ("seconds", values("seconds")),
            ("plan_seconds", values("plan_seconds")),
            ("apply_seconds", values("apply_seconds")),
            ("answer_seconds", values("answer_seconds")),
        ]));
    }
    let candidate = |unit: &str| {
        results
            .iter()
            .find(|r| {
                r.field("label").as_str() == Some("candidate")
                    && r.field("unit").as_str() == Some(unit)
            })
            .and_then(|r| r.field("seconds").as_f64())
    };
    let below = match (
        candidate(&System::Todo.unit("llvm")),
        candidate(&System::Todo.unit("rust")),
    ) {
        (Some(llvm), Some(rust)) => Json::Bool(llvm < rust),
        _ => Json::Null,
    };
    obj(vec![
        ("schema", SCHEMA.into()),
        ("samples", SAMPLES.into()),
        ("warmup", WARMUP.into()),
        ("results", Json::Arr(results)),
        ("todo_llvm_below_rust", below),
    ])
}

/// The self-check cases of the apply cycle, on fixture texts and records.
pub fn self_check() -> R<()> {
    ensure(
        units("baseline") == ["apply-cycle-terminal-rust", "apply-cycle-terminal-llvm"],
        "apply-cycle: the before tree cycles the terminal design only (D12)",
    )?;
    ensure(
        units("candidate")
            == [
                "apply-cycle-terminal-rust",
                "apply-cycle-todo-rust",
                "apply-cycle-terminal-llvm",
                "apply-cycle-todo-llvm",
            ],
        "apply-cycle: the after tree cycles the terminal design and the todo",
    )?;
    let commands = "  Status => Reply { lines = [\"ready\"] }\n";
    let edited = replace_once(
        commands,
        &System::Terminal.literal("ready"),
        &System::Terminal.literal(&System::Terminal.text(1)),
    )?;
    ensure(
        edited == "  Status => Reply { lines = [\"ready 1\"] }\n",
        "apply-cycle: the terminal's status reply is the edited literal",
    )?;
    let route = "  (\"GET\", [\"health\"]) => Response.plain(200, \"ok\\n\")\n";
    let edited = replace_once(
        route,
        &System::Todo.literal("ok"),
        &System::Todo.literal(&System::Todo.text(2)),
    )?;
    ensure(
        edited == "  (\"GET\", [\"health\"]) => Response.plain(200, \"ok 2\\n\")\n",
        "apply-cycle: the todo's health body is the edited literal",
    )?;
    ensure(
        replace_once("", "port = 2323", "port = 1").is_err()
            && replace_once("port = 2323 port = 2323", "port = 2323", "port = 1").is_err(),
        "apply-cycle: a missing or repeated literal is refused",
    )?;
    ensure(
        phase(0) == "warmup" && (WARMUP..WARMUP + SAMPLES).all(|n| phase(n) == "measured"),
        "apply-cycle: one discarded warm-up, then the measured samples",
    )?;
    let sample = |label: &str, unit: &str, phase: &str, seconds: f64, exit: i64| {
        obj(vec![
            ("label", label.into()),
            ("unit", unit.into()),
            ("phase", phase.into()),
            ("seconds", seconds.into()),
            ("plan_seconds", (seconds / 2.0).into()),
            ("apply_seconds", (seconds / 4.0).into()),
            ("answer_seconds", (seconds / 4.0).into()),
            ("exit", exit.into()),
        ])
    };
    let mut rows = vec![sample(
        "candidate",
        "apply-cycle-todo-llvm",
        "warmup",
        100.0,
        0,
    )];
    for seconds in [1.0, 2.0, 3.0] {
        rows.push(sample(
            "candidate",
            "apply-cycle-todo-llvm",
            "measured",
            seconds,
            0,
        ));
        rows.push(sample(
            "candidate",
            "apply-cycle-todo-rust",
            "measured",
            seconds * 4.0,
            0,
        ));
    }
    rows.push(sample(
        "candidate",
        "apply-cycle-todo-llvm",
        "measured",
        50.0,
        1,
    ));
    let value = summary(&rows);
    let llvm = &value.field("results").as_arr()[0];
    ensure(
        llvm.field("unit").as_str() == Some("apply-cycle-todo-llvm")
            && llvm.field("samples") == &Json::Int(3)
            && llvm.field("failed") == &Json::Int(1)
            && llvm.field("seconds") == &Json::Float(2.0)
            && llvm.field("plan_seconds") == &Json::Float(1.0),
        "apply-cycle: medians leave out the warm-up and failed samples",
    )?;
    ensure(
        value.field("todo_llvm_below_rust") == &Json::Bool(true),
        "apply-cycle: row 28 compares the todo's LLVM and Rust medians",
    )?;
    ensure(
        summary(&rows[..1]).field("todo_llvm_below_rust") == &Json::Null,
        "apply-cycle: no observation without measured samples",
    )
}

/// One tree's cycle of one system on one target: its working directory, its host and the text
/// the server answers now.
struct Cycle {
    label: String,
    system: System,
    target: &'static str,
    compiler: PathBuf,
    runtime: PathBuf,
    workdir: PathBuf,
    state: PathBuf,
    port: u16,
    text: String,
    host: Option<Child>,
    host_log: PathBuf,
}

fn wait(child: &mut Child, limit: Duration) -> R<Option<i64>> {
    let deadline = Instant::now() + limit;
    loop {
        if let Some(status) = child.try_wait().map_err(io_err("wait"))? {
            return Ok(Some(exit_code(status)));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// A port nobody listens on now, for the copy's listener.
fn free_port() -> R<u16> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(io_err("bind"))?;
    Ok(listener.local_addr().map_err(io_err("bind"))?.port())
}

/// Read from `stream` until `done` holds for what was read or the stream ends.
fn read_until(stream: &mut TcpStream, done: impl Fn(&[u8]) -> bool) -> Vec<u8> {
    let mut received = Vec::new();
    let mut bytes = [0; 4096];
    while !done(&received) {
        match stream.read(&mut bytes) {
            Ok(0) | Err(_) => break,
            Ok(count) => received.extend_from_slice(&bytes[..count]),
        }
    }
    received
}

fn connect(port: u16) -> Option<TcpStream> {
    let stream =
        TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), Duration::from_secs(5)).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .ok()?;
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .ok()?;
    Some(stream)
}

/// Whether the server on `port` answers `text`: the terminal's `status` reply line, the todo's
/// `GET /health` body.
fn answers(system: System, port: u16, text: &str) -> bool {
    let Some(mut stream) = connect(port) else {
        return false;
    };
    match system {
        System::Terminal => {
            let prompt = |b: &[u8]| b.ends_with(b"terminal> ");
            if !prompt(&read_until(&mut stream, prompt)) {
                return false;
            }
            if stream.write_all(b"status\r\n").is_err() {
                return false;
            }
            let reply = read_until(&mut stream, prompt);
            let _ = stream.write_all(b"quit\r\n");
            String::from_utf8_lossy(&reply)
                .lines()
                .any(|line| line.trim() == text)
        }
        System::Todo => {
            let request = "GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n";
            if stream.write_all(request.as_bytes()).is_err() {
                return false;
            }
            let complete = |b: &[u8]| {
                let Some(end) = b.windows(4).position(|w| w == b"\r\n\r\n") else {
                    return false;
                };
                let head = String::from_utf8_lossy(&b[..end]).to_ascii_lowercase();
                let length = head
                    .lines()
                    .find_map(|l| l.strip_prefix("content-length:"))
                    .and_then(|v| v.trim().parse::<usize>().ok());
                length.is_some_and(|n| b.len() >= end + 4 + n)
            };
            let response = read_until(&mut stream, complete);
            let text_response = String::from_utf8_lossy(&response);
            let Some((head, body)) = text_response.split_once("\r\n\r\n") else {
                return false;
            };
            head.starts_with("HTTP/1.1 200") && body == format!("{text}\n")
        }
    }
}

impl Harness {
    /// Build the compiler and the debug runner runtime of every tree, then cycle each target
    /// and system with the trees interleaved per sample, and write `apply-cycle.json`.
    pub fn apply_cycle(&mut self) -> R<()> {
        let pair = format!("{}-apply-cycle-prepare", self.capture_id);
        for label in self.labels() {
            let target = self.side(&label).target.clone();
            let build = words(&["cargo", "build", "-q", "-p", "ignite"]);
            let row = self.command(
                &label,
                "apply-cycle-compiler",
                &pair,
                build,
                false,
                "prepare",
            )?;
            ensure(
                !row.field("exit").truthy(),
                format!("apply-cycle: the compiler of {label} did not build"),
            )?;
            let rt = path_str(&target.join("ignite-test/rt"));
            let runtime = words(&[
                "cargo",
                "build",
                "-q",
                "-p",
                "ignite-rt",
                "--features",
                "runner",
                "--target-dir",
                &rt,
            ]);
            let row = self.command(&label, "runtime-dev", &pair, runtime, false, "prepare")?;
            ensure(
                !row.field("exit").truthy(),
                format!("apply-cycle: the runtime of {label} did not build"),
            )?;
        }
        let mut outcome = Ok(());
        'targets: for target in TARGETS {
            for system in [System::Terminal, System::Todo] {
                let labels: Vec<String> = self
                    .labels()
                    .into_iter()
                    .filter(|label| systems(label).contains(&system))
                    .collect();
                let mut cycles = Vec::new();
                let mut result = Ok(());
                for label in &labels {
                    match self.cycle_start(label, system, target) {
                        Ok(cycle) => cycles.push(cycle),
                        Err(error) => {
                            result = Err(error);
                            break;
                        }
                    }
                }
                if result.is_ok() {
                    result = self.cycle_samples(&mut cycles);
                }
                for cycle in &mut cycles {
                    let stopped = self.cycle_stop(cycle);
                    if result.is_ok() {
                        result = stopped;
                    }
                }
                if result.is_err() {
                    outcome = result;
                    break 'targets;
                }
            }
        }
        crate::perf::dump(
            &self.evidence.join("apply-cycle.json"),
            &summary(&self.rows),
        )?;
        outcome
    }

    fn cycle_samples(&mut self, cycles: &mut [Cycle]) -> R<()> {
        for number in 0..WARMUP + SAMPLES {
            let pair = format!("{}-apply-cycle-{number}", self.capture_id);
            for cycle in cycles.iter_mut() {
                let text = cycle.system.text(number);
                let row = self.cycle_step(cycle, &pair, phase(number), Some(text))?;
                ensure(
                    !row.field("exit").truthy(),
                    format!(
                        "apply-cycle: sample {number} of {} {} failed; see {}",
                        cycle.label,
                        cycle.system.unit(cycle.target),
                        row.field("log").as_str().unwrap_or("")
                    ),
                )?;
            }
        }
        Ok(())
    }

    /// A temporary working directory under the build root with a copy of the example on a free
    /// port, `ignite host` started in it and revision 1 applied and answering.
    fn cycle_start(&mut self, label: &str, system: System, target: &'static str) -> R<Cycle> {
        let side = self.side(label).clone();
        let workdir = self
            .builds
            .join("apply-cycle")
            .join(&self.capture_id)
            .join(format!("{label}-{}-{target}", system.name()));
        let copy = workdir.join(system.directory());
        fs::create_dir_all(&copy).map_err(io_err(copy.display()))?;
        let original = side.path.join(system.directory());
        for entry in fs::read_dir(&original).map_err(io_err(original.display()))? {
            let path = entry.map_err(io_err(original.display()))?.path();
            if path.extension().is_some_and(|e| e == "ignite") {
                let name = path.file_name().expect("file name");
                fs::copy(&path, copy.join(name)).map_err(io_err(path.display()))?;
            }
        }
        let port = free_port()?;
        let source = copy.join(system.source());
        let text = fs::read_to_string(&source).map_err(io_err(source.display()))?;
        let text = replace_once(&text, system.port_text(), &format!("port = {port}"))?;
        fs::write(&source, text).map_err(io_err(source.display()))?;
        let state = workdir.join("state");
        fs::create_dir_all(&state).map_err(io_err(state.display()))?;
        let logs = self.evidence.join("logs");
        fs::create_dir_all(&logs).map_err(io_err(logs.display()))?;
        let host_log = logs.join(format!(
            "{}-apply-cycle-{label}-{}-host.log",
            self.capture_id,
            system.unit(target)
        ));
        let mut cycle = Cycle {
            label: label.to_string(),
            system,
            target,
            compiler: side.target.join("debug/ignite"),
            runtime: side.target.join("ignite-test/rt/debug/libignite_rt.a"),
            workdir,
            state,
            port,
            text: system.original().to_string(),
            host: None,
            host_log,
        };
        let stream = fs::File::create(&cycle.host_log).map_err(io_err(cycle.host_log.display()))?;
        let errors = stream
            .try_clone()
            .map_err(io_err(cycle.host_log.display()))?;
        let host = Command::new(&cycle.compiler)
            .arg("host")
            .arg("--state")
            .arg(&cycle.state)
            .current_dir(&cycle.workdir)
            .env_clear()
            .envs(&self.env_base)
            .stdin(Stdio::piped())
            .stdout(stream)
            .stderr(errors)
            .spawn()
            .map_err(io_err(cycle.compiler.display()))?;
        cycle.host = Some(host);
        let online = self.host_online(&mut cycle);
        let setup = online.and_then(|_| {
            let pair = format!("{}-apply-cycle-setup", self.capture_id);
            let row = self.cycle_step(&mut cycle, &pair, "prepare", None)?;
            ensure(
                !row.field("exit").truthy(),
                format!(
                    "apply-cycle: revision 1 of {label} {} failed; see {}",
                    system.unit(target),
                    row.field("log").as_str().unwrap_or("")
                ),
            )
        });
        if let Err(error) = setup {
            let _ = self.cycle_stop(&mut cycle);
            return Err(error);
        }
        Ok(cycle)
    }

    fn host_online(&self, cycle: &mut Cycle) -> R<()> {
        let deadline = Instant::now() + LIVE;
        loop {
            let host = cycle.host.as_mut().expect("host started");
            if let Some(status) = host.try_wait().map_err(io_err("host"))? {
                return Err(format!(
                    "apply-cycle: ignite host exited {}; see {}",
                    exit_code(status),
                    cycle.host_log.display()
                ));
            }
            let output = Command::new(&cycle.compiler)
                .arg("inspect")
                .arg("--state")
                .arg(&cycle.state)
                .arg("--json")
                .current_dir(&cycle.workdir)
                .env_clear()
                .envs(&self.env_base)
                .stdin(Stdio::null())
                .stderr(Stdio::null())
                .output()
                .map_err(io_err("inspect"))?;
            let online = Json::parse(&String::from_utf8_lossy(&output.stdout))
                .is_ok_and(|status| status.field("online") == &Json::Bool(true));
            if online {
                return Ok(());
            }
            ensure(
                Instant::now() < deadline,
                format!(
                    "apply-cycle: ignite host not online after {LIVE:?}; see {}",
                    cycle.host_log.display()
                ),
            )?;
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Run `args` of the cycle's compiler in its working directory, output to `log`; the exit
    /// status, or `None` past `limit` (the process is killed).
    fn cycle_run(&self, cycle: &Cycle, args: &[String], log: &Path, limit: Duration) -> R<i64> {
        let stream = fs::File::create(log).map_err(io_err(log.display()))?;
        let errors = stream.try_clone().map_err(io_err(log.display()))?;
        let mut child = Command::new(&cycle.compiler)
            .args(args)
            .current_dir(&cycle.workdir)
            .env_clear()
            .envs(&self.env_base)
            .stdin(Stdio::null())
            .stdout(stream)
            .stderr(errors)
            .spawn()
            .map_err(io_err(cycle.compiler.display()))?;
        match wait(&mut child, limit)? {
            Some(code) => Ok(code),
            None => {
                let _ = child.kill();
                let _ = child.wait();
                let mut file = fs::OpenOptions::new()
                    .append(true)
                    .open(log)
                    .map_err(io_err(log.display()))?;
                writeln!(file, "ignite-perf: killed after {limit:?}").map_err(io_err("log"))?;
                Ok(-9)
            }
        }
    }

    /// One plan, apply and first answer. With `text`, the response literal changes to it first;
    /// without, the unedited source is applied (revision 1).
    fn cycle_step(
        &mut self,
        cycle: &mut Cycle,
        pair: &str,
        phase: &str,
        text: Option<String>,
    ) -> R<Json> {
        self.guard()?;
        let system = cycle.system;
        let unit = system.unit(cycle.target);
        let new_text = text.unwrap_or_else(|| cycle.text.clone());
        if new_text != cycle.text {
            let edited = cycle.workdir.join(system.directory()).join(system.edited());
            let source = fs::read_to_string(&edited).map_err(io_err(edited.display()))?;
            let source = replace_once(
                &source,
                &system.literal(&cycle.text),
                &system.literal(&new_text),
            )?;
            fs::write(&edited, source).map_err(io_err(edited.display()))?;
        }
        let logs = self.evidence.join("logs");
        let stem = format!("{}-{pair}-{}-{unit}", self.capture_id, cycle.label);
        let plan_log = logs.join(format!("{stem}-plan.log"));
        let apply_log = logs.join(format!("{stem}-apply.log"));
        let plan_file = cycle.workdir.join(format!("{pair}.plan.json"));
        let source = cycle.workdir.join(system.directory()).join(system.source());
        let mut plan = vec![
            "plan".to_string(),
            path_str(&source),
            "--assembly".into(),
            "local".into(),
            "--for-apply".into(),
            "--state".into(),
            path_str(&cycle.state),
            "--out".into(),
            path_str(&plan_file),
            "--target".into(),
            cycle.target.into(),
        ];
        if cycle.target == "llvm" {
            plan.push("--runtime".into());
            plan.push(path_str(&cycle.runtime));
        }
        let apply = vec![
            "apply".to_string(),
            path_str(&plan_file),
            "--state".into(),
            path_str(&cycle.state),
        ];
        let load_before = loadavg()?;
        let start = Instant::now();
        let mut exit = self.cycle_run(cycle, &plan, &plan_log, BUILD)?;
        let plan_seconds = start.elapsed().as_secs_f64();
        let mut apply_seconds = 0.0;
        let mut answer_seconds = 0.0;
        if exit == 0 {
            let applied = Instant::now();
            exit = self.cycle_run(cycle, &apply, &apply_log, APPLY)?;
            apply_seconds = applied.elapsed().as_secs_f64();
            let receipt = fs::read_to_string(&apply_log).unwrap_or_default();
            let completed =
                Json::parse(&receipt).is_ok_and(|r| r.field("state").as_str() == Some("completed"));
            if exit == 0 && !completed {
                exit = 1;
            }
        }
        if exit == 0 {
            let asked = Instant::now();
            while !answers(system, cycle.port, &new_text) {
                if asked.elapsed() >= LIVE {
                    exit = 1;
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            answer_seconds = asked.elapsed().as_secs_f64();
        }
        let seconds = start.elapsed().as_secs_f64();
        if exit == 0 {
            cycle.text = new_text;
        }
        let relative = |path: &Path| {
            path_str(
                path.strip_prefix(&self.evidence)
                    .expect("log under evidence"),
            )
        };
        let mut command = vec![path_str(&cycle.compiler)];
        command.extend(plan);
        let side = self.side(&cycle.label).clone();
        let row = obj(vec![
            ("schema", SCHEMA.into()),
            ("label", cycle.label.as_str().into()),
            ("commit", side.commit.as_str().into()),
            ("unit", unit.as_str().into()),
            ("pair", pair.into()),
            ("phase", phase.into()),
            ("cache_policy", "normal".into()),
            ("command", command.into()),
            ("seconds", seconds.into()),
            ("plan_seconds", plan_seconds.into()),
            ("apply_seconds", apply_seconds.into()),
            ("answer_seconds", answer_seconds.into()),
            ("text", cycle.text.as_str().into()),
            ("exit", exit.into()),
            ("load_before", load_before.into()),
            ("load_after", loadavg()?.into()),
            ("log", relative(&plan_log).into()),
            ("apply_log", relative(&apply_log).into()),
        ]);
        self.append(row)
    }

    /// `stop` on the host's control input; the working directory is removed once the host
    /// exited 0, and kept for inspection otherwise.
    fn cycle_stop(&self, cycle: &mut Cycle) -> R<()> {
        let Some(mut host) = cycle.host.take() else {
            return Ok(());
        };
        if let Some(stdin) = host.stdin.as_mut() {
            let _ = writeln!(stdin, "stop");
            let _ = stdin.flush();
        }
        let status = wait(&mut host, LIVE)?;
        if status.is_none() {
            let _ = host.kill();
            let _ = host.wait();
        }
        ensure(
            status == Some(0),
            format!(
                "apply-cycle: ignite host stop gave {status:?}; kept {}; see {}",
                cycle.workdir.display(),
                cycle.host_log.display()
            ),
        )?;
        fs::remove_dir_all(&cycle.workdir).map_err(io_err(cycle.workdir.display()))
    }
}

fn words(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}
