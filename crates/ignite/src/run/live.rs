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
//! `<member>.rollout` (`Control("rollout start_first")` for the control line
//! `rollout <member> start_first`), `Control("stop")` to every `Supervise` and `Roll`
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
//!
//! Transitions (`docs/slice.md` 17.4, a local host only, never `ignite
//! run`): [`Live::with_transitions`] lets a host's transition driver change
//! the running revision member by member. Members in `deferred` get no
//! `Start` at segment start and no event until the control line `start
//! <member>`, which starts their machines and replays the target's
//! readiness to a scheduler whose target is already ready. `retire
//! <member>` drains the member's process with `Control("stop")` (SIGKILL
//! after its drain deadline) and stops feeding its other machines.
//! `rebind` copies the builder's artifacts into the next generation, so
//! later spawns and probe attempts use the desired revision's executables.
//! `restore <member>...` gives each member back the live instance its newest
//! process displaced (a start-first replacement that failed) and kills the
//! member's other processes. `handover <member>...` stops every process of a member not named, then
//! ends this segment without stopping the named ones and returns the
//! adapters, the channel and the pinned ports ([`Ended::Handover`]): the
//! next revision's run adopts those instances, and a `Spawn` of an adopted
//! member is answered with the surviving instance's `Started` (and `Ready`
//! when it was ready) instead of a second process.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
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

/// A host receives each record before its effect can be dispatched. Returning
/// an error stops the live driver, including when durable host storage fails.
pub type RecordSink = Box<dyn FnMut(&Record) -> io::Result<()> + Send>;

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
            // Every resource an executable creates provisions it (a
            // `Store`, the capabilities of 21.8); only a listener has a
            // port to pin.
            let Some(resource) = model
                .member(&edge.from)
                .filter(|resource| resource.kind == "TcpListener")
            else {
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

/// The live process of one member: its pid, the generation it was spawned
/// with, whether it printed `listening on` and the port it reported.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Instance {
    pub pid: i32,
    pub generation: i64,
    pub ready: bool,
    pub port: u16,
}

/// What a run hands to the next revision's run of the same host: the
/// adapters whose spawner thread is the parent of the surviving children,
/// the observation channel they report to, the pinned ports of the
/// surviving listeners and the live instance of each surviving member.
#[derive(Debug)]
pub struct Carried {
    pub adapters: Adapters,
    pub tx: Sender<Observation>,
    pub rx: Receiver<Observation>,
    pub pinned: BTreeMap<String, u16>,
    pub instances: BTreeMap<String, Instance>,
    /// Control lines that arrived during the handover; the next run
    /// receives them first.
    pub pending: Vec<Observation>,
}

/// How a run ended.
#[derive(Debug)]
pub enum Ended {
    /// The stop sequence ran; the exit status of 16.3.
    Stopped(i32),
    /// A host's `handover` (module doc).
    Handover(Carried),
}

/// The transition state of a host's run (module doc).
#[derive(Default)]
struct Transitions {
    enabled: bool,
    deferred: BTreeSet<String>,
    retired: BTreeSet<usize>,
    adopted: BTreeMap<String, Instance>,
    handover: Option<BTreeSet<String>>,
    /// The lowest generation a `Spawn` uses: the generation whose
    /// executables are the desired revision's.
    floor: i64,
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
    /// Synchronous acknowledgements of this driver's own Spawn effects.
    /// Complete the current reducer/effect turn before delivering them.
    completions: VecDeque<Observation>,
    deferred: VecDeque<Observation>,
    record_sink: Option<RecordSink>,
    started_hook: Option<Box<dyn FnOnce() -> io::Result<()> + Send>>,
    transitions: Transitions,
    /// Member to its newest live process.
    instances: BTreeMap<String, Instance>,
    /// Member to the live instance its newest process displaced, until that
    /// instance ends: what `restore <member>` gives back.
    displaced: BTreeMap<String, Instance>,
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
            completions: VecDeque::new(),
            deferred: VecDeque::new(),
            record_sink: None,
            started_hook: None,
            transitions: Transitions::default(),
            instances: BTreeMap::new(),
            displaced: BTreeMap::new(),
        }
    }

    /// Accept a host transition driver's control lines (module doc):
    /// `deferred` members start only on `start <member>`, `adopted` are the
    /// live instances a previous revision's run handed over, `pinned` its
    /// pinned ports of their listeners.
    pub fn with_transitions(
        mut self,
        deferred: BTreeSet<String>,
        adopted: BTreeMap<String, Instance>,
        pinned: BTreeMap<String, u16>,
    ) -> Self {
        self.transitions = Transitions {
            enabled: true,
            deferred,
            floor: self.generation,
            adopted,
            ..Transitions::default()
        };
        self.pinned = pinned;
        self
    }

    /// Whether `machine` gets no event: its member is deferred or the
    /// machine retired.
    fn blocked(&self, machine: usize) -> bool {
        self.transitions.retired.contains(&machine)
            || self
                .core
                .machines()
                .get(machine)
                .is_some_and(|m| self.transitions.deferred.contains(&m.member))
    }

    /// Attach an owning host's durable revision journal without changing the
    /// existing run journal format or the reducer's dispatch ordering.
    pub fn with_record_sink(mut self, sink: RecordSink) -> Self {
        self.record_sink = Some(sink);
        self
    }

    /// Notify a host after every initial machine turn has been dispatched.
    /// This hook adds no record to an existing run journal.
    pub fn with_started_hook(mut self, hook: Box<dyn FnOnce() -> io::Result<()> + Send>) -> Self {
        self.started_hook = Some(hook);
        self
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
        self.run_loop()?;
        self.stop()
    }

    /// A host's run: like [`Live::run`], or the handover of the instances
    /// that stay to the next revision's run (module doc).
    pub fn run_host(mut self) -> io::Result<Ended> {
        self.run_loop()?;
        match self.transitions.handover.take() {
            Some(keep) => self.handover(keep).map(Ended::Handover),
            None => self.stop().map(Ended::Stopped),
        }
    }

    /// The segment, `Start` to every machine, the loop until a stop or a
    /// handover is requested.
    fn run_loop(&mut self) -> io::Result<()> {
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
            if self.blocked(index) {
                continue;
            }
            let at = self.now();
            let start = kind(
                "Start",
                vec![Value::Quantity(QuantityKind::Duration, self.config.epoch)],
            );
            let records = self.core.feed(index, at, start);
            self.process(records)?;
        }
        if let Some(hook) = self.started_hook.take() {
            hook()?;
        }
        while !self.stopping {
            if !self.completions.is_empty() {
                if !self.config.manual {
                    self.fire_due()?;
                }
                self.complete_spawns()?;
            }
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
                        self.complete_spawns()?;
                        continue;
                    }
                    Some(due) => match self
                        .rx
                        .recv_timeout(Duration::from_nanos((due - now) as u64))
                    {
                        Ok(observation) => observation,
                        Err(RecvTimeoutError::Timeout) => {
                            self.fire_due()?;
                            self.complete_spawns()?;
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
            self.observe_next(received, false)?;
            if self.transitions.handover.is_some() {
                break;
            }
        }
        Ok(())
    }

    /// At a turn boundary, publish the acknowledgements already known from
    /// synchronous dispatch before starting an unrelated observation's work.
    fn complete_spawns(&mut self) -> io::Result<()> {
        while let Some(observation) = self.completions.pop_front() {
            self.observe(observation, false)?;
        }
        Ok(())
    }

    fn observe_next(&mut self, observation: Observation, advancing: bool) -> io::Result<()> {
        self.complete_spawns()?;
        self.observe(observation, advancing)
    }

    /// Write and dispatch records in order: the journal first (synced for
    /// an effect), then stdout with `--json`, stderr always, then the
    /// effect's dispatch.
    fn process(&mut self, records: Vec<Record>) -> io::Result<()> {
        for record in records {
            let line = record.line();
            self.journal.append(&line, record.durable)?;
            if let Some(sink) = &mut self.record_sink {
                sink(&record)?;
            }
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

    /// Pin the port `member`'s ready process reported for its listener
    /// (16.3), unless the listener is configured with another fixed port:
    /// an instance a previous revision handed over still listens on that
    /// revision's port, which is not the desired one.
    fn pin(&mut self, member: &str, port: u16) {
        let Some(listener) = self.members.listeners.get(member) else {
            return;
        };
        let configured = self
            .config
            .ports
            .get(&listener.resource)
            .copied()
            .unwrap_or(listener.port);
        if configured == 0 || configured == port {
            self.pinned.insert(listener.resource.clone(), port);
        }
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
                if let Some(instance) = self.transitions.adopted.remove(&member)
                    && self
                        .adapters
                        .pids(&member)
                        .iter()
                        .any(|(_, pid)| *pid == instance.pid)
                {
                    self.completions.push_back(Observation::Started {
                        member: member.clone(),
                        generation: instance.generation,
                        pid: instance.pid,
                    });
                    if instance.ready && self.members.listeners.contains_key(&member) {
                        let address = self
                            .members
                            .listeners
                            .get(&member)
                            .map(|l| l.address.clone())
                            .unwrap_or_default();
                        self.completions.push_back(Observation::Ready {
                            member,
                            pid: instance.pid,
                            address,
                            port: instance.port,
                        });
                    }
                    return;
                }
                let generation = generation.max(self.transitions.floor);
                let Some(executable) = self.executable(&member, generation) else {
                    self.completions.push_back(Observation::SpawnFailed {
                        reason: format!("{member} has no executable"),
                        member,
                    });
                    return;
                };
                if !executable.is_file() {
                    self.completions.push_back(Observation::SpawnFailed {
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
                let completions = &mut self.completions;
                self.adapters.spawn_with_completion(
                    &member,
                    generation,
                    &executable,
                    &args,
                    listener.is_some(),
                    &mut |observation| completions.push_back(observation),
                );
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
            if self.blocked(machine) {
                continue;
            }
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
            if self.blocked(*machine) {
                continue;
            }
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
            Observation::StopBoundary => Ok(()),
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
                if let Some(previous) = self.instances.get(&member).copied()
                    && previous.pid != pid
                    && self
                        .adapters
                        .pids(&member)
                        .iter()
                        .any(|(_, live)| *live == previous.pid)
                {
                    self.displaced.insert(member.clone(), previous);
                }
                self.instances.insert(
                    member.clone(),
                    Instance {
                        pid,
                        generation,
                        ready: false,
                        port: 0,
                    },
                );
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
                self.pin(&member, port);
                if let Some(instance) = self.instances.get_mut(&member)
                    && instance.pid == pid
                {
                    instance.ready = true;
                    instance.port = port;
                }
                let targets = self.lifecycle(&member);
                self.feed_all(&targets, kind("Ready", vec![Value::Int(i64::from(pid))]))
            }
            Observation::Exited { member, pid, code } => {
                self.ended(&member, pid);
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
                self.ended(&member, pid);
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
                let line = match words.next() {
                    Some("start_first") => "rollout start_first",
                    _ => "rollout",
                };
                match self.core.machine(&id) {
                    Some(machine) => self.feed_all(
                        &[machine],
                        kind("Control", vec![Value::Text(line.to_string())]),
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
            Some(
                word @ ("start" | "retire" | "retire-deadline" | "rebind" | "restore" | "handover"),
            ) if self.transitions.enabled => {
                let rest: Vec<String> = words.map(str::to_string).collect();
                self.transition_control(word, rest)
            }
            Some(other) => {
                self.note(&format!("unknown control line `{other}`"));
                Ok(())
            }
            None => Ok(()),
        }
    }

    /// The control lines of a host's transition driver (module doc).
    fn transition_control(&mut self, word: &str, rest: Vec<String>) -> io::Result<()> {
        match (word, rest.as_slice()) {
            ("start", [member]) => self.start_member(member),
            ("retire", [member]) => self.retire(member),
            ("retire-deadline", [member, pids @ ..]) => {
                let live: Vec<i32> = self
                    .adapters
                    .pids(member)
                    .into_iter()
                    .map(|(_, pid)| pid)
                    .collect();
                for pid in pids.iter().filter_map(|pid| pid.parse::<i32>().ok()) {
                    if live.contains(&pid) {
                        self.note(&format!(
                            "{member} pid {pid} still running at its drain deadline"
                        ));
                        let _ = self.adapters.kill(pid, SIGKILL);
                    }
                }
                Ok(())
            }
            ("rebind", []) => {
                self.generation += 1;
                let bin = self
                    .config
                    .state
                    .join("bin")
                    .join(self.generation.to_string());
                let built = (self.config.builder)().map_err(io::Error::other)?;
                copy_into(&bin, &built)?;
                self.transitions.floor = self.generation;
                Ok(())
            }
            ("restore", members) => {
                for member in members {
                    self.restore(member);
                }
                Ok(())
            }
            ("handover", keep) => {
                self.transitions.handover = Some(keep.iter().cloned().collect());
                Ok(())
            }
            _ => {
                self.note(&format!("malformed control line `{word}`"));
                Ok(())
            }
        }
    }

    /// `start <member>`: the member's machines get `Start`, and a
    /// scheduler whose target is already ready gets that `Ready`.
    fn start_member(&mut self, member: &str) -> io::Result<()> {
        self.transitions.deferred.remove(member);
        let machines: Vec<usize> = self
            .core
            .machines()
            .iter()
            .enumerate()
            .filter(|(_, m)| m.member == member)
            .map(|(i, _)| i)
            .collect();
        for index in &machines {
            self.transitions.retired.remove(index);
        }
        self.feed_all(
            &machines,
            kind(
                "Start",
                vec![Value::Quantity(QuantityKind::Duration, self.config.epoch)],
            ),
        )?;
        let targets: Vec<String> = self
            .model
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Readiness && e.from == member)
            .map(|e| e.to.clone())
            .collect();
        let schedulers: Vec<usize> = machines
            .iter()
            .copied()
            .filter(|i| self.core.machines()[*i].variant() == "Schedule")
            .collect();
        for target in targets {
            if let Some(instance) = self.instances.get(&target).filter(|i| i.ready) {
                let pid = instance.pid;
                self.feed_all(&schedulers, kind("Ready", vec![Value::Int(i64::from(pid))]))?;
            }
        }
        Ok(())
    }

    /// `retire <member>`: its processes drain under `Control("stop")` and
    /// get SIGKILL at the member's drain deadline; its other machines get
    /// no further event.
    fn retire(&mut self, member: &str) -> io::Result<()> {
        let mut drain = Vec::new();
        for (index, machine) in self.core.machines().iter().enumerate() {
            if machine.member != member {
                continue;
            }
            match machine.variant() {
                "Supervise" | "Roll" => drain.push(index),
                _ => {
                    self.transitions.retired.insert(index);
                }
            }
        }
        self.feed_all(
            &drain,
            kind("Control", vec![Value::Text("stop".to_string())]),
        )?;
        let pids: Vec<String> = self
            .adapters
            .pids(member)
            .into_iter()
            .map(|(_, pid)| pid.to_string())
            .collect();
        if !pids.is_empty() {
            let deadline = self.member_deadline(member);
            let tx = self.tx.clone();
            let line = format!("retire-deadline {member} {}", pids.join(" "));
            std::thread::spawn(move || {
                std::thread::sleep(deadline);
                let _ = tx.send(Observation::Control(line));
            });
        }
        Ok(())
    }

    fn ended(&mut self, member: &str, pid: i32) {
        if self.instances.get(member).is_some_and(|i| i.pid == pid) {
            self.instances.remove(member);
        }
        if self.displaced.get(member).is_some_and(|i| i.pid == pid) {
            self.displaced.remove(member);
        }
    }

    /// `restore <member>`: the instance the member's newest process
    /// displaced is the member's live instance again, and every other
    /// process of the member gets SIGKILL, so that a `handover` carries the
    /// displaced instance (module doc).
    fn restore(&mut self, member: &str) {
        let Some(instance) = self.displaced.remove(member) else {
            return;
        };
        let live = self.adapters.pids(member);
        if !live.iter().any(|(_, pid)| *pid == instance.pid) {
            return;
        }
        for (_, pid) in live {
            if pid != instance.pid {
                let _ = self.adapters.kill(pid, SIGKILL);
            }
        }
        self.instances.insert(member.to_string(), instance);
    }

    /// End this segment for the next revision's run: stop every process of
    /// a member not in `keep`, wait for them as the stop sequence does, and
    /// hand the rest over (module doc).
    fn handover(mut self, keep: BTreeSet<String>) -> io::Result<Carried> {
        self.complete_spawns()?;
        let targets: Vec<usize> = self
            .core
            .machines()
            .iter()
            .enumerate()
            .filter(|(_, m)| {
                matches!(m.variant(), "Supervise" | "Roll") && !keep.contains(&m.member)
            })
            .map(|(i, _)| i)
            .collect();
        self.feed_all(
            &targets,
            kind("Control", vec![Value::Text("stop".to_string())]),
        )?;
        let leaving: Vec<String> = self
            .config
            .executables
            .keys()
            .filter(|member| !keep.contains(*member))
            .cloned()
            .collect();
        let mut pending = Vec::new();
        self.await_exits(&leaving, &mut pending)?;
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
        while let Some(observation) = self.deferred.pop_front() {
            pending.push(observation);
        }
        let resources: BTreeSet<String> = keep
            .iter()
            .filter_map(|member| self.members.listeners.get(member))
            .map(|listener| listener.resource.clone())
            .collect();
        let pinned = self
            .pinned
            .iter()
            .filter(|(resource, _)| resources.contains(*resource))
            .map(|(resource, port)| (resource.clone(), *port))
            .collect();
        let live: BTreeSet<(String, i32)> = keep
            .iter()
            .flat_map(|member| {
                self.adapters
                    .pids(member)
                    .into_iter()
                    .map(move |(_, pid)| (member.clone(), pid))
            })
            .collect();
        let instances = self
            .instances
            .iter()
            .filter(|(member, instance)| live.contains(&((*member).clone(), instance.pid)))
            .map(|(member, instance)| (member.clone(), *instance))
            .collect();
        Ok(Carried {
            adapters: self.adapters,
            tx: self.tx,
            rx: self.rx,
            pinned,
            instances,
            pending,
        })
    }

    /// The deadline a drain of `member` waits: its `DrainRollout.deadline`,
    /// 10 s without one.
    fn member_deadline(&self, member: &str) -> Duration {
        self.core
            .machines_of(member, "Roll")
            .into_iter()
            .find_map(|index| {
                let machine = &self.core.machines()[index];
                let policy = machine.field("policy")?;
                let ns = field(policy, "deadline").and_then(int)?;
                u64::try_from(ns).ok().map(Duration::from_nanos)
            })
            .unwrap_or(STOP_DEADLINE)
    }

    /// Feed only `Exited` and `Signalled` until no child of `members` is
    /// left or the longest of their deadlines passes, SIGKILL what is left
    /// and wait once more. Control lines that arrive meanwhile go to
    /// `pending`; lifecycle observations of other members update their
    /// instances only.
    fn await_exits(
        &mut self,
        members: &[String],
        pending: &mut Vec<Observation>,
    ) -> io::Result<()> {
        let deadline = members
            .iter()
            .filter(|member| !self.adapters.pids(member).is_empty())
            .map(|member| self.member_deadline(member))
            .max()
            .unwrap_or(Duration::ZERO);
        let started = Instant::now();
        let mut killed = false;
        loop {
            let live: Vec<i32> = members
                .iter()
                .flat_map(|member| self.adapters.pids(member))
                .map(|(_, pid)| pid)
                .collect();
            if live.is_empty() {
                let _ = self.tx.send(Observation::StopBoundary);
                while let Ok(observation) = self.rx.try_recv() {
                    if matches!(observation, Observation::StopBoundary) {
                        break;
                    }
                    self.during_handover(observation, members, pending)?;
                }
                return Ok(());
            }
            let budget = if killed { AFTER_KILL } else { deadline };
            let elapsed = started.elapsed();
            if elapsed >= budget {
                if killed {
                    return Ok(());
                }
                for pid in live {
                    let _ = self.adapters.kill(pid, SIGKILL);
                }
                killed = true;
                continue;
            }
            match self.rx.recv_timeout(budget - elapsed) {
                Ok(observation) => self.during_handover(observation, members, pending)?,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => return Ok(()),
            }
        }
    }

    fn during_handover(
        &mut self,
        observation: Observation,
        leaving: &[String],
        pending: &mut Vec<Observation>,
    ) -> io::Result<()> {
        match observation {
            Observation::Exited { ref member, .. } | Observation::Signalled { ref member, .. }
                if leaving.contains(member) =>
            {
                self.observe(observation, false)
            }
            Observation::Exited { member, pid, .. }
            | Observation::Signalled { member, pid, .. } => {
                self.ended(&member, pid);
                Ok(())
            }
            Observation::Started {
                member,
                generation,
                pid,
            } => {
                self.instances.insert(
                    member,
                    Instance {
                        pid,
                        generation,
                        ready: false,
                        port: 0,
                    },
                );
                Ok(())
            }
            Observation::Ready {
                member, pid, port, ..
            } => {
                if let Some(instance) = self.instances.get_mut(&member)
                    && instance.pid == pid
                {
                    instance.ready = true;
                    instance.port = port;
                }
                self.pin(&member, port);
                Ok(())
            }
            observation @ (Observation::Control(_) | Observation::Interrupt) => {
                pending.push(observation);
                Ok(())
            }
            _ => Ok(()),
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
            self.observe_next(observation, false)?;
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
            self.complete_spawns()?;
            self.adapters.settle(self.config.settle);
            let mut any = false;
            while let Ok(observation) = self.rx.try_recv() {
                any = true;
                self.observe_next(observation, true)?;
            }
            if any {
                continue;
            }
            match self.rx.recv_timeout(Duration::from_millis(20)) {
                Ok(observation) => self.observe_next(observation, true)?,
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
                writeln!(
                    out,
                    "declared, not buildable: {declared} (its kind has no main)"
                )?;
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
        self.complete_spawns()?;
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
                // Process waiters publish completion under the table lock
                // before retiring their pid. Every final exit is therefore
                // already queued when this snapshot becomes empty. Fence that
                // finite prefix so new control input cannot extend shutdown.
                let _ = self.tx.send(Observation::StopBoundary);
                while let Ok(observation) = self.rx.try_recv() {
                    match observation {
                        Observation::StopBoundary => break,
                        Observation::Exited { .. } | Observation::Signalled { .. } => {
                            self.observe(observation, false)?;
                        }
                        _ => {}
                    }
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use ignite_eval::{Evaluator, Fuel};
    use std::sync::mpsc::channel;

    /// Seed recorded history through the real reducers, as replay does. Its
    /// historical effects have already happened and are not dispatched again.
    fn history(core: &mut Core<'_, '_>, journal: &mut Journal, machine: usize, event: Value) {
        for record in core.feed(machine, 0, event) {
            journal.append(&record.line(), record.durable).unwrap();
        }
    }

    #[test]
    fn timer_spawn_completion_precedes_a_held_probe_result() {
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/terminal/terminal.design.ignite");
        let loaded = crate::load(&source, None).unwrap();
        let model = loaded.model(&crate::PlanOptions::default()).unwrap();
        let modules = loaded.modules();
        let types = modules.get(CONTROL_MODULE).unwrap().types;
        let mut evaluator = Evaluator::new(&modules, Fuel::default());
        let mut core = Core::new(&mut evaluator, &model);
        let state =
            std::env::temp_dir().join(format!("ignite-live-completion-{}", std::process::id()));
        let bin = state.join("bin/1");
        std::fs::create_dir_all(&bin).unwrap();
        std::os::unix::fs::symlink("/bin/true", bin.join("server")).unwrap();
        let mut journal = Journal::open(&state, true).unwrap();
        let model_json = Json::parse(&model.to_json()).unwrap();
        journal
            .append(&crate::journal_header(&model_json, 0), true)
            .unwrap();
        for machine in 0..core.machines().len() {
            history(
                &mut core,
                &mut journal,
                machine,
                kind("Start", vec![Value::Quantity(QuantityKind::Duration, 0)]),
            );
        }
        let supervisor = core.machine("Terminal.server.supervisor").unwrap();
        let rollout = core.machine("Terminal.server.rollout").unwrap();
        let scheduler = core.machine("Terminal.promptProbe.scheduler").unwrap();
        for event in [
            kind("Started", vec![Value::Int(1234), Value::Int(1)]),
            kind("Ready", vec![Value::Int(1234)]),
            kind("Signalled", vec![Value::Int(1234), Value::Int(9)]),
        ] {
            for machine in [supervisor, rollout, scheduler] {
                history(&mut core, &mut journal, machine, event.clone());
            }
        }
        let config = Config {
            state: state.clone(),
            manual: true,
            epoch: 0,
            settle: Duration::from_secs(1),
            json: false,
            ports: BTreeMap::new(),
            declared: Vec::new(),
            executables: BTreeMap::from([("Terminal.server".to_string(), "server".to_string())]),
            generation: 1,
            start_at: 0,
            builder: Arc::new(|| Ok(BTreeMap::new())),
        };
        let (tx, rx) = channel();
        let adapters = Adapters::start(tx.clone()).unwrap();
        let mut live = Live::new(core, &model, types, config, journal, adapters, tx, rx);
        // This models the real-clock branch that already received an external
        // observation, then fires a due timer before delivering that message.
        let held = Observation::AttemptDone {
            member: "Terminal.promptProbe".to_string(),
            id: 1,
            line: "{\"fields\":{\"latency\":{\"ns\":0,\"type\":\"Duration\"},\"received\":false},\"type\":\"std.sli.ProbeAttempt\"}".to_string(),
        };
        live.manual.advance_to(100_000_000);
        live.fire_due().unwrap();
        live.observe_next(held, false).unwrap();
        while let Ok(observation) = live.rx.try_recv() {
            live.observe_next(observation, false).unwrap();
        }
        assert_eq!(live.stop().unwrap(), 0);
        drop(live);
        let lines: Vec<String> = std::fs::read_to_string(state.join("journal.jsonl"))
            .unwrap()
            .lines()
            .map(str::to_string)
            .collect();
        let records: Vec<Json> = lines
            .iter()
            .map(|line| Json::parse(line).unwrap())
            .collect();
        let event = |record: &Json| {
            record
                .get("value")
                .and_then(|v| v.get("fields"))
                .and_then(|f| f.get("kind"))
                .and_then(|k| k.get("variant"))
                .and_then(Json::as_str)
                .map(str::to_string)
        };
        let at = |record: &Json| {
            record
                .get("at")
                .and_then(|v| v.get("ns"))
                .and_then(Json::as_i64)
        };
        let started: Vec<_> = records
            .iter()
            .enumerate()
            .filter(|(_, r)| event(r).as_deref() == Some("Started") && at(r) == Some(100_000_000))
            .collect();
        assert_eq!(
            started
                .iter()
                .map(|(_, r)| r.get("machine").unwrap().as_str().unwrap())
                .collect::<Vec<_>>(),
            [
                "Terminal.server.supervisor",
                "Terminal.server.rollout",
                "Terminal.promptProbe.scheduler"
            ]
        );
        let probe = records
            .iter()
            .position(|r| event(r).as_deref() == Some("AttemptDone"))
            .unwrap();
        assert!(
            started.last().unwrap().0 < probe,
            "complete lifecycle acknowledgement before probe fanout"
        );
        let published: Vec<_> = records
            .iter()
            .enumerate()
            .filter(|(_, record)| {
                record.get("kind").and_then(Json::as_str) == Some("effect")
                    && record
                        .get("value")
                        .and_then(|v| v.get("variant"))
                        .and_then(Json::as_str)
                        == Some("Publish")
            })
            .collect();
        assert_eq!(
            published.len(),
            3,
            "the held probe still publishes all standings"
        );
        assert!(published.iter().all(|(index, _)| *index > probe));
        let mut replay_core = Core::new(&mut evaluator, &model);
        super::super::replay::replay(&mut replay_core, types, &lines[1..]).unwrap();
        std::fs::remove_dir_all(state).unwrap();
    }
}
