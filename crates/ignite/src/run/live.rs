//! The live run (`docs/slice.md` 16.2 to 16.4, 16.9): one thread owns the
//! [`Core`], the journal and the clock, receives every adapter observation
//! over one channel, stamps it with `at`, routes it as a `std.control`
//! event (A4), writes the records the core produces and dispatches the
//! effects to the [`Adapters`].
//!
//! Routing: a lifecycle observation of member `M` (`Started`, `Ready`,
//! `Exited`, `Signalled`, `SpawnFailed`) reaches `M`'s `Supervise` and
//! `Roll` machines and every `Schedule` machine whose member has a
//! `readiness` edge to `M`, in machine-id order; a member without a
//! `provisioning` listener gets a synthetic `Ready` right after `Started`.
//! `AttemptDone` and `AttemptFailed` go to the scheduler that asked for
//! the attempt, `Built`, `BuildFailed` and `Released` to the rollout
//! machine that asked, `Timer` to its setter, `Control("rollout")` to
//! `<member>.rollout`, `Control("stop")` to every `Supervise` and `Roll`
//! machine. `status` and `advance` reach no machine.
//!
//! Time (16.4): under the real clock `at` is the segment's start plus the
//! monotonic clock, and every timer due before an observation fires first.
//! Under the manual clock `at` moves only on `advance`: at each instant the
//! run settles (waits in real time until nothing is in flight, at most
//! `--settle`), processes what arrived, then fires the next due timer at
//! its own instant, and finally moves `now` to the target. Control lines
//! that arrive while advancing are handled after it.
//!
//! Stop (16.2): `Control("stop")` is stepped, the kills it produces are
//! dispatched, then only `Exited` and `Signalled` are fed until no child
//! is left or the deadline passes (each member's `DrainRollout.deadline`,
//! 10 s without one), what is left gets SIGKILL, and the stop `segment`
//! closes the journal. Exit 1 when a machine failed or gave up.

use std::collections::{BTreeMap, VecDeque};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use ignite_eval::{CONTROL_MODULE, EdgeKind, Json, Model, Value, value_json};
use ignite_types::{QualName, QuantityKind, Reference, Ty, TypedProgram};

use super::clock::{Manual, Real, parse_duration};
use super::control::{Adapters, Observation};
use super::engine::{Core, Dispatch, Record, field, int, kind, segment};
use super::journal::Journal;

/// Builds every buildable member into the build directory and returns
/// member id to the built executable (16.3); a rollout calls it again.
pub type Builder = Arc<dyn Fn() -> Result<BTreeMap<String, PathBuf>, String> + Send + Sync>;

/// What a run is given besides the model.
pub struct Config {
    /// The state directory: `bin/<generation>/` holds the executables.
    pub state: PathBuf,
    /// `--clock manual`.
    pub manual: bool,
    /// The journal's epoch, nanoseconds since 1970-01-01T00:00:00Z.
    pub epoch: i64,
    /// The settle limit (16.4).
    pub settle: Duration,
    /// `--json`: records to stdout.
    pub json: bool,
    /// `--port RESOURCE=PORT`.
    pub ports: BTreeMap<String, u16>,
    /// Members declared but not buildable (A10), for the segment and
    /// `status`.
    pub declared: Vec<String>,
    /// Member id to executable file name, for every member that runs or is
    /// built.
    pub executables: BTreeMap<String, String>,
    /// The generation this segment starts with.
    pub generation: i64,
    /// The `at` this segment starts at (0 for a fresh journal).
    pub start_at: i64,
    pub builder: Builder,
}

/// The listener a member provisions: the resource and its declared
/// address and port.
#[derive(Clone, Debug)]
struct Listener {
    resource: String,
    address: String,
    port: u16,
}

/// What the run reads from the model about members.
struct Members {
    listeners: BTreeMap<String, Listener>,
    /// Member to the members its `Ref` properties name, in property order.
    refs: BTreeMap<String, Vec<String>>,
}

impl Members {
    fn of(model: &Model) -> Members {
        let mut listeners = BTreeMap::new();
        for edge in &model.edges {
            if edge.kind != EdgeKind::Provisioning {
                continue;
            }
            let Some(resource) = model.member(&edge.from) else {
                continue;
            };
            let property = |name: &str| resource.properties.iter().find(|(f, _)| f == name);
            let address = property("address")
                .and_then(|(_, v)| match v {
                    Value::Text(text) => Some(text.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| "127.0.0.1".to_string());
            let port = property("port")
                .and_then(|(_, v)| int(v))
                .and_then(|p| u16::try_from(p).ok())
                .unwrap_or(0);
            listeners.entry(edge.to.clone()).or_insert(Listener {
                resource: edge.from.clone(),
                address,
                port,
            });
        }
        let mut refs = BTreeMap::new();
        for member in &model.members {
            let mut targets = Vec::new();
            for (_, value) in &member.properties {
                match value {
                    Value::Reference(Reference::Member { system, member }) => {
                        targets.push(format!("{system}.{member}"));
                    }
                    Value::Member { system, member } => {
                        targets.push(format!("{system}.{member}"));
                    }
                    _ => {}
                }
            }
            if !targets.is_empty() {
                refs.insert(member.id.clone(), targets);
            }
        }
        Members { listeners, refs }
    }
}

/// One live run.
pub struct Live<'a, 'p> {
    core: Core<'a, 'p>,
    model: &'a Model,
    types: &'a TypedProgram,
    attempt_ty: Ty,
    config: Config,
    members: Members,
    journal: Journal,
    adapters: Adapters,
    tx: Sender<Observation>,
    rx: Receiver<Observation>,
    real: Real,
    manual: Manual,
    /// Resource id to the port its member's child reported (16.3).
    pinned: BTreeMap<String, u16>,
    generation: i64,
    stopping: bool,
    deferred: VecDeque<Observation>,
}

const SIGKILL: i32 = 9;
const STOP_DEADLINE: Duration = Duration::from_secs(10);
const AFTER_KILL: Duration = Duration::from_secs(2);

impl<'a, 'p> Live<'a, 'p> {
    /// A run over `core`, whose machines may carry a resumed state.
    /// `types` is the typed program of `std.control`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        core: Core<'a, 'p>,
        model: &'a Model,
        types: &'a TypedProgram,
        config: Config,
        journal: Journal,
        adapters: Adapters,
        tx: Sender<Observation>,
        rx: Receiver<Observation>,
    ) -> Live<'a, 'p> {
        let attempt_ty = types
            .enum_variants(&QualName::new(CONTROL_MODULE, "EventKind"))
            .and_then(|variants| {
                variants
                    .iter()
                    .find(|(name, _)| name == "AttemptDone")
                    .and_then(|(_, payload)| payload.get(1).cloned())
            })
            .unwrap_or_else(|| Ty::Record(QualName::new("std.sli", "ProbeAttempt")));
        let members = Members::of(model);
        let manual = Manual {
            now: config.start_at,
        };
        Live {
            core,
            model,
            types,
            attempt_ty,
            generation: config.generation,
            config,
            members,
            journal,
            adapters,
            tx,
            rx,
            real: Real::start(),
            manual,
            pinned: BTreeMap::new(),
            stopping: false,
            deferred: VecDeque::new(),
        }
    }

    fn now(&self) -> i64 {
        if self.config.manual {
            self.manual.now
        } else {
            self.config.start_at.saturating_add(self.real.now())
        }
    }

    fn clock_name(&self) -> &'static str {
        if self.config.manual { "manual" } else { "real" }
    }

    fn note(&self, text: &str) {
        eprintln!("ignite run: {text}");
    }

    /// The whole run: the segment, `Start` to every machine, the loop,
    /// the stop sequence; the exit status of 16.3.
    pub fn run(mut self) -> io::Result<i32> {
        let at = self.now();
        let value = segment(
            at,
            None,
            self.clock_name(),
            self.generation,
            &self.config.declared,
        );
        let record = self
            .core
            .record("segment", at, None, value_json(&value), true, None);
        self.process(vec![record])?;
        for index in 0..self.core.machines().len() {
            let at = self.now();
            let start = kind(
                "Start",
                vec![Value::Quantity(QuantityKind::Duration, self.config.epoch)],
            );
            let records = self.core.feed(index, at, start);
            self.process(records)?;
        }
        while !self.stopping {
            let received = if self.config.manual {
                match self.rx.recv() {
                    Ok(observation) => observation,
                    Err(_) => break,
                }
            } else {
                let now = self.now();
                match self.core.next_timer_at() {
                    Some(due) if due <= now => {
                        self.fire_due()?;
                        continue;
                    }
                    Some(due) => match self
                        .rx
                        .recv_timeout(Duration::from_nanos((due - now) as u64))
                    {
                        Ok(observation) => observation,
                        Err(RecvTimeoutError::Timeout) => {
                            self.fire_due()?;
                            continue;
                        }
                        Err(RecvTimeoutError::Disconnected) => break,
                    },
                    None => match self.rx.recv() {
                        Ok(observation) => observation,
                        Err(_) => break,
                    },
                }
            };
            if !self.config.manual {
                self.fire_due()?;
            }
            self.observe(received, false)?;
        }
        self.stop()
    }

    /// Write and dispatch records in order: the journal first (synced for
    /// an effect), then stdout with `--json`, stderr always, then the
    /// effect's dispatch.
    fn process(&mut self, records: Vec<Record>) -> io::Result<()> {
        for record in records {
            let line = record.line();
            self.journal.append(&line, record.durable)?;
            if self.config.json {
                let stdout = io::stdout();
                let mut out = stdout.lock();
                writeln!(out, "{line}")?;
                out.flush()?;
            }
            eprintln!("{}", record.human());
            if let Some(dispatch) = record.dispatch {
                self.dispatch(dispatch);
            }
        }
        Ok(())
    }

    fn executable(&self, member: &str, generation: i64) -> Option<PathBuf> {
        let name = self.config.executables.get(member)?;
        Some(
            self.config
                .state
                .join("bin")
                .join(generation.to_string())
                .join(name),
        )
    }

    fn port_of(&self, listener: &Listener) -> u16 {
        self.pinned
            .get(&listener.resource)
            .or_else(|| self.config.ports.get(&listener.resource))
            .copied()
            .unwrap_or(listener.port)
    }

    /// `<address>:<port>` of the listener `member` provisions (16.9).
    fn endpoint_of(&self, member: &str) -> Option<String> {
        let listener = self.members.listeners.get(member)?;
        Some(format!("{}:{}", listener.address, self.port_of(listener)))
    }

    /// `--ref <path>=<endpoint>` for every `Ref` property of `member` whose
    /// target provisions a listener.
    fn refs_of(&self, member: &str) -> Vec<(String, String)> {
        self.members
            .refs
            .get(member)
            .map(|targets| {
                targets
                    .iter()
                    .filter_map(|target| Some((target.clone(), self.endpoint_of(target)?)))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn dispatch(&mut self, dispatch: Dispatch) {
        match dispatch {
            Dispatch::Spawn { member, generation } => {
                let Some(executable) = self.executable(&member, generation) else {
                    let _ = self.tx.send(Observation::SpawnFailed {
                        reason: format!("{member} has no executable"),
                        member,
                    });
                    return;
                };
                if !executable.is_file() {
                    let _ = self.tx.send(Observation::SpawnFailed {
                        reason: format!("{} is not built", executable.display()),
                        member,
                    });
                    return;
                }
                let mut args = Vec::new();
                let listener = self.members.listeners.get(&member).cloned();
                if let Some(listener) = &listener {
                    args.push("--port".to_string());
                    args.push(self.port_of(listener).to_string());
                }
                for (path, endpoint) in self.refs_of(&member) {
                    args.push("--ref".to_string());
                    args.push(format!("{path}={endpoint}"));
                }
                self.adapters
                    .spawn(&member, generation, &executable, &args, listener.is_some());
            }
            Dispatch::Kill {
                member,
                pid,
                signal,
            } => {
                if let Err(error) = self.adapters.kill(pid, signal) {
                    self.note(&format!("kill {signal} of {member} pid {pid}: {error}"));
                }
            }
            Dispatch::Abort { member, id } => self.adapters.kill_attempt(&member, id),
            Dispatch::Attempt { member, id } => {
                let Some(executable) = self
                    .executable(&member, self.generation)
                    .filter(|path| path.is_file())
                else {
                    let _ = self.tx.send(Observation::AttemptFailed {
                        reason: format!("{member} has no built executable"),
                        member,
                        id,
                    });
                    return;
                };
                let refs = self.refs_of(&member);
                self.adapters.attempt(&member, id, &executable, &refs);
            }
            Dispatch::Build { member } => {
                self.generation += 1;
                let generation = self.generation;
                let guard = self.adapters.in_flight();
                let builder = self.config.builder.clone();
                let tx = self.tx.clone();
                let bin = self.config.state.join("bin").join(generation.to_string());
                std::thread::spawn(move || {
                    let _guard = guard;
                    let result = builder()
                        .and_then(|built| copy_into(&bin, &built).map_err(|e| e.to_string()));
                    let _ = tx.send(match result {
                        Ok(()) => Observation::Built { member, generation },
                        Err(reason) => Observation::BuildFailed { member, reason },
                    });
                });
            }
            Dispatch::AwaitRelease { member } => {
                if !self.adapters.await_release(&member) {
                    self.note(&format!("{member} has no live process to release"));
                }
            }
        }
    }

    /// Fire every timer due at or before now, each at its own instant.
    fn fire_due(&mut self) -> io::Result<()> {
        let now = self.now();
        while let Some((at, machine, id)) = self.core.next_due(now) {
            let records = self
                .core
                .feed(machine, at, kind("Timer", vec![Value::Int(id)]));
            self.process(records)?;
        }
        Ok(())
    }

    /// The machines a lifecycle observation of `member` reaches (module
    /// doc): the member's supervisor, then its rollout, then the
    /// schedulers of the probes whose readiness edge points at it, in id
    /// order within a group.
    fn lifecycle(&self, member: &str) -> Vec<usize> {
        let probes: Vec<&str> = self
            .model
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Readiness && e.to == member)
            .map(|e| e.from.as_str())
            .collect();
        let machines = self.core.machines();
        let of_kind = |variant: &'static str| {
            machines
                .iter()
                .enumerate()
                .filter(move |(_, m)| m.variant() == variant)
        };
        let supervisors = of_kind("Supervise").filter(|(_, m)| m.member == member);
        let rollouts = of_kind("Roll").filter(|(_, m)| m.member == member);
        let schedulers = of_kind("Schedule").filter(|(_, m)| probes.contains(&m.member.as_str()));
        supervisors
            .chain(rollouts)
            .chain(schedulers)
            .map(|(i, _)| i)
            .collect()
    }

    fn feed_all(&mut self, machines: &[usize], kind: Value) -> io::Result<()> {
        let at = self.now();
        for machine in machines {
            let records = self.core.feed(*machine, at, kind.clone());
            self.process(records)?;
        }
        Ok(())
    }

    fn feed_one(
        &mut self,
        machine: Option<usize>,
        fallback: Vec<usize>,
        kind: Value,
    ) -> io::Result<()> {
        match machine {
            Some(machine) => self.feed_all(&[machine], kind),
            None => self.feed_all(&fallback, kind),
        }
    }

    /// One observation at the current instant. While advancing, control
    /// lines and interrupts wait until the advance is done.
    fn observe(&mut self, observation: Observation, advancing: bool) -> io::Result<()> {
        match observation {
            Observation::Control(_) | Observation::Interrupt if advancing => {
                self.deferred.push_back(observation);
                Ok(())
            }
            Observation::Control(line) => self.control(&line),
            Observation::Interrupt => {
                self.stopping = true;
                Ok(())
            }
            Observation::Started {
                member,
                generation,
                pid,
            } => {
                let targets = self.lifecycle(&member);
                self.feed_all(
                    &targets,
                    kind(
                        "Started",
                        vec![Value::Int(i64::from(pid)), Value::Int(generation)],
                    ),
                )?;
                if !self.members.listeners.contains_key(&member) {
                    let _ = self.tx.send(Observation::Ready {
                        member,
                        pid,
                        address: String::new(),
                        port: 0,
                    });
                }
                Ok(())
            }
            Observation::Ready {
                member, pid, port, ..
            } => {
                if let Some(listener) = self.members.listeners.get(&member) {
                    self.pinned.insert(listener.resource.clone(), port);
                }
                let targets = self.lifecycle(&member);
                self.feed_all(&targets, kind("Ready", vec![Value::Int(i64::from(pid))]))
            }
            Observation::Exited { member, pid, code } => {
                let targets = self.lifecycle(&member);
                self.feed_all(
                    &targets,
                    kind(
                        "Exited",
                        vec![Value::Int(i64::from(pid)), Value::Int(i64::from(code))],
                    ),
                )
            }
            Observation::Signalled {
                member,
                pid,
                signal,
            } => {
                let targets = self.lifecycle(&member);
                self.feed_all(
                    &targets,
                    kind(
                        "Signalled",
                        vec![Value::Int(i64::from(pid)), Value::Int(i64::from(signal))],
                    ),
                )
            }
            Observation::SpawnFailed { member, reason } => {
                let targets = self.lifecycle(&member);
                self.feed_all(&targets, kind("SpawnFailed", vec![Value::Text(reason)]))
            }
            Observation::AttemptDone { member, id, line } => {
                let scheduler = self.core.take_attempt(&member, id);
                let fallback = self.core.machines_of(&member, "Schedule");
                match Value::decode(&line, &self.attempt_ty, self.types) {
                    Ok(attempt) => self.feed_one(
                        scheduler,
                        fallback,
                        kind("AttemptDone", vec![Value::Int(id), attempt]),
                    ),
                    Err(error) => self.feed_one(
                        scheduler,
                        fallback,
                        kind(
                            "AttemptFailed",
                            vec![
                                Value::Int(id),
                                Value::Text(format!("undecodable probe line: {error}")),
                            ],
                        ),
                    ),
                }
            }
            Observation::AttemptFailed { member, id, reason } => {
                let scheduler = self.core.take_attempt(&member, id);
                let fallback = self.core.machines_of(&member, "Schedule");
                self.feed_one(
                    scheduler,
                    fallback,
                    kind("AttemptFailed", vec![Value::Int(id), Value::Text(reason)]),
                )
            }
            Observation::Released { member, .. } => {
                let machine = self.core.take_release(&member);
                let fallback = self.core.machines_of(&member, "Roll");
                self.feed_one(machine, fallback, kind("Released", vec![]))
            }
            Observation::Built { member, generation } => {
                let machine = self.core.take_build(&member);
                let fallback = self.core.machines_of(&member, "Roll");
                self.feed_one(
                    machine,
                    fallback,
                    kind("Built", vec![Value::Int(generation)]),
                )
            }
            Observation::BuildFailed { member, reason } => {
                let machine = self.core.take_build(&member);
                let fallback = self.core.machines_of(&member, "Roll");
                self.feed_one(
                    machine,
                    fallback,
                    kind("BuildFailed", vec![Value::Text(reason)]),
                )
            }
        }
    }

    /// One control line (16.3).
    fn control(&mut self, line: &str) -> io::Result<()> {
        let mut words = line.split_whitespace();
        match words.next() {
            Some("status") => self.status(),
            Some("stop") => {
                self.stopping = true;
                Ok(())
            }
            Some("rollout") => {
                let Some(member) = words.next() else {
                    self.note("rollout needs a member");
                    return Ok(());
                };
                let id = format!("{member}.rollout");
                match self.core.machine(&id) {
                    Some(machine) => self.feed_all(
                        &[machine],
                        kind("Control", vec![Value::Text("rollout".to_string())]),
                    ),
                    None => {
                        self.note(&format!("{member} has no rollout machine"));
                        Ok(())
                    }
                }
            }
            Some("advance") => {
                let quantity: Vec<&str> = words.collect();
                let quantity = quantity.join(" ");
                if !self.config.manual {
                    self.note("advance needs --clock manual");
                    return Ok(());
                }
                match parse_duration(&quantity) {
                    Some(d) => self.advance(d),
                    None => {
                        self.note(&format!("advance: `{quantity}` is no Duration literal"));
                        Ok(())
                    }
                }
            }
            Some(other) => {
                self.note(&format!("unknown control line `{other}`"));
                Ok(())
            }
            None => Ok(()),
        }
    }

    /// `advance d` (16.4, module doc).
    fn advance(&mut self, d: i64) -> io::Result<()> {
        let target = self.manual.now.saturating_add(d);
        self.settle_and_drain()?;
        while let Some((at, machine, id)) = self.core.next_due(target) {
            self.manual.advance_to(at);
            let records = self
                .core
                .feed(machine, at, kind("Timer", vec![Value::Int(id)]));
            self.process(records)?;
            self.settle_and_drain()?;
        }
        self.manual.advance_to(target);
        while let Some(observation) = self.deferred.pop_front() {
            self.observe(observation, false)?;
            if self.stopping {
                break;
            }
        }
        Ok(())
    }

    /// The settle rule at one instant: wait until nothing is in flight (at
    /// most the limit), process what arrived, and repeat while anything
    /// arrives. An observation sent just before its in-flight guard is
    /// released is caught by one short wait.
    fn settle_and_drain(&mut self) -> io::Result<()> {
        loop {
            self.adapters.settle(self.config.settle);
            let mut any = false;
            while let Ok(observation) = self.rx.try_recv() {
                any = true;
                self.observe(observation, true)?;
            }
            if any {
                continue;
            }
            match self.rx.recv_timeout(Duration::from_millis(20)) {
                Ok(observation) => self.observe(observation, true)?,
                Err(_) => return Ok(()),
            }
        }
    }

    /// `status` (16.3): the standing of every contract, the state of every
    /// machine, the declared members; one JSON line with `--json`.
    fn status(&mut self) -> io::Result<()> {
        let now = self.now();
        let standings = self.core.current_standings(now);
        let stdout = io::stdout();
        let mut out = stdout.lock();
        if self.config.json {
            let mut object = BTreeMap::new();
            object.insert(
                "declared".to_string(),
                Json::Array(
                    self.config
                        .declared
                        .iter()
                        .map(|d| Json::Str(d.clone()))
                        .collect(),
                ),
            );
            object.insert("kind".to_string(), Json::Str("status".to_string()));
            object.insert(
                "ports".to_string(),
                Json::Object(
                    self.pinned
                        .iter()
                        .map(|(resource, port)| (resource.clone(), Json::Int(i64::from(*port))))
                        .collect(),
                ),
            );
            object.insert(
                "machines".to_string(),
                Json::Array(
                    self.core
                        .machines()
                        .iter()
                        .map(|m| {
                            let mut entry = BTreeMap::new();
                            entry.insert("id".to_string(), Json::Str(m.id.clone()));
                            entry.insert("machine".to_string(), value_json(&m.value));
                            Json::Object(entry)
                        })
                        .collect(),
                ),
            );
            object.insert(
                "standings".to_string(),
                Json::Array(standings.iter().map(|s| value_json(&s.value)).collect()),
            );
            writeln!(out, "{}", Json::Object(object).compact())?;
        } else {
            for standing in &standings {
                writeln!(
                    out,
                    "standing {}: {}",
                    standing.contract,
                    standing.value.debug_text()
                )?;
            }
            for machine in self.core.machines() {
                writeln!(
                    out,
                    "machine {}: {}",
                    machine.id,
                    machine.value.debug_text()
                )?;
            }
            for (resource, port) in &self.pinned {
                writeln!(out, "port {resource}: {port}")?;
            }
            for declared in &self.config.declared {
                writeln!(out, "declared, not buildable in slice 12: {declared}")?;
            }
        }
        out.flush()
    }

    /// The longest deadline a stop waits for its children: each member's
    /// `DrainRollout.deadline` from its rollout machine, 10 s without one.
    fn stop_deadline(&self) -> Duration {
        let mut deadline = Duration::ZERO;
        for member in self.config.executables.keys() {
            if self.adapters.pids(member).is_empty() {
                continue;
            }
            let own = self
                .core
                .machines_of(member, "Roll")
                .into_iter()
                .find_map(|index| {
                    let machine = &self.core.machines()[index];
                    let policy = machine.field("policy")?;
                    let ns = field(policy, "deadline").and_then(int)?;
                    u64::try_from(ns).ok().map(Duration::from_nanos)
                })
                .unwrap_or(STOP_DEADLINE);
            deadline = deadline.max(own);
        }
        deadline
    }

    /// The stop sequence (module doc).
    fn stop(&mut self) -> io::Result<i32> {
        let targets: Vec<usize> = self
            .core
            .machines()
            .iter()
            .enumerate()
            .filter(|(_, m)| matches!(m.variant(), "Supervise" | "Roll"))
            .map(|(i, _)| i)
            .collect();
        self.feed_all(
            &targets,
            kind("Control", vec![Value::Text("stop".to_string())]),
        )?;
        let deadline = self.stop_deadline();
        let started = Instant::now();
        let mut killed = false;
        loop {
            let live: Vec<i32> = self
                .config
                .executables
                .keys()
                .flat_map(|member| self.adapters.pids(member))
                .map(|(_, pid)| pid)
                .collect();
            if live.is_empty() {
                break;
            }
            let budget = if killed { AFTER_KILL } else { deadline };
            let elapsed = started.elapsed();
            if elapsed >= budget {
                if killed {
                    break;
                }
                for pid in live {
                    let _ = self.adapters.kill(pid, SIGKILL);
                }
                killed = true;
                continue;
            }
            match self.rx.recv_timeout(budget - elapsed) {
                Ok(observation @ (Observation::Exited { .. } | Observation::Signalled { .. })) => {
                    self.observe(observation, false)?;
                }
                Ok(_) => {}
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
        let at = self.now();
        let value = segment(
            self.config.start_at,
            Some(at),
            self.clock_name(),
            self.generation,
            &self.config.declared,
        );
        let record = self
            .core
            .record("segment", at, None, value_json(&value), true, None);
        self.process(vec![record])?;
        Ok(if self.core.failed() { 1 } else { 0 })
    }
}

/// Copy the built executables into `bin`, keeping their file names.
pub fn copy_into(bin: &Path, built: &BTreeMap<String, PathBuf>) -> io::Result<()> {
    std::fs::create_dir_all(bin)?;
    for path in built.values() {
        let Some(name) = path.file_name() else {
            continue;
        };
        let target = bin.join(name);
        let temporary = bin.join(format!(".{}.tmp", name.to_string_lossy()));
        std::fs::copy(path, &temporary)?;
        std::fs::rename(&temporary, &target)?;
    }
    Ok(())
}
