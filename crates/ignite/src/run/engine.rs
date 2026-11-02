//! The engine (`docs/slice.md` 16.2, 16.6, 16.9): the deterministic core
//! that `ignite run` and `ignite replay` share (16.15). [`Core`] owns the
//! machines of one assembly, steps `std.control.step` in the evaluator for
//! every event it is fed, and turns what the reducers return into journal
//! records in order: the `event` record first, then one `effect` record
//! per effect, each followed by the record its dispatch writes (`signal`,
//! `standing`, `consequence`) and, for an emission, the `Emitted` events
//! of the subscribers, which it steps after the current step's effects.
//! Timers, in-flight attempts, pending builds and releases are tables of
//! the core, rebuilt by replay from the same records. A reducer failure is
//! an `error` record (E128): the machine stops and the run goes on.
//!
//! The core knows the `std.control` types and nothing of any domain
//! (invariant 6): a member is a path text, a signal a ref text, and the
//! only fields it reads are those `std.control` declares (`id`, `state`,
//! `version`, `standing_seq`, `contract`, `period_from`).

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use ignite_eval::{CONTROL_MODULE, EvalError, Evaluator, Json, Limit, Model, Value, value_json};
use ignite_types::{QualName, QuantityKind};

use super::Standing;

/// The `formatVersion` of the journal (16.5).
pub const JOURNAL_VERSION: &str = "ignite-journal/0.1";

/// One journal record (16.5) before it is written: `seq`, `at`, the kind,
/// the machine it belongs to, the value, whether it is synced before the
/// engine goes on (O30) and what the engine dispatches after writing it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub seq: u64,
    pub at: i64,
    pub kind: &'static str,
    pub machine: Option<String>,
    pub value: Json,
    pub durable: bool,
    pub dispatch: Option<Dispatch>,
}

impl Record {
    /// The journal line without its newline: keys in byte order, compact.
    pub fn line(&self) -> String {
        let mut object = BTreeMap::new();
        object.insert("at".to_string(), duration_json(self.at));
        object.insert("kind".to_string(), Json::Str(self.kind.to_string()));
        object.insert(
            "machine".to_string(),
            self.machine
                .as_ref()
                .map_or(Json::Null, |m| Json::Str(m.clone())),
        );
        object.insert("seq".to_string(), Json::Int(self.seq as i64));
        object.insert("value".to_string(), self.value.clone());
        Json::Object(object).compact()
    }

    /// The one human line of 16.3 for stderr.
    pub fn human(&self) -> String {
        let mut line = format!(
            "{} {} {}",
            self.seq,
            QuantityKind::Duration.debug_text(self.at),
            self.kind
        );
        if let Some(machine) = &self.machine {
            line.push(' ');
            line.push_str(machine);
        }
        line.push_str(": ");
        line.push_str(&summary(&self.value));
        line
    }
}

/// A short text of a record's value: an event's kind, an effect's variant
/// and payload, a standing's status and counts.
fn summary(value: &Json) -> String {
    fn variant(value: &Json) -> Option<String> {
        let variant = value.get("variant")?.as_str()?;
        let fields = value
            .get("fields")
            .and_then(Json::as_array)
            .map(|items| items.iter().map(brief).collect::<Vec<_>>().join(", "))
            .unwrap_or_default();
        Some(if fields.is_empty() {
            variant.to_string()
        } else {
            format!("{variant}({fields})")
        })
    }
    fn brief(value: &Json) -> String {
        match value {
            Json::Str(text) => format!("{text:?}"),
            Json::Int(n) => n.to_string(),
            Json::Bool(b) => b.to_string(),
            Json::Null => "()".to_string(),
            Json::Array(items) => format!(
                "[{}]",
                items.iter().map(brief).collect::<Vec<_>>().join(", ")
            ),
            Json::Object(object) => {
                if let (Some(Json::Str(ty)), Some(Json::Int(n))) = (
                    object.get("type"),
                    object
                        .get("ns")
                        .or(object.get("ppm"))
                        .or(object.get("bytes")),
                ) {
                    return match ty.as_str() {
                        "Duration" => QuantityKind::Duration.debug_text(*n),
                        "Ratio" => QuantityKind::Ratio.debug_text(*n),
                        _ => QuantityKind::ByteSize.debug_text(*n),
                    };
                }
                if let Some(v) = variant(value) {
                    return v;
                }
                if let Some(fields) = value.get("fields").and_then(Json::as_object) {
                    return format!(
                        "{{ {} }}",
                        fields
                            .iter()
                            .map(|(k, v)| format!("{k}: {}", brief(v)))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
                format!(
                    "{{ {} }}",
                    object
                        .iter()
                        .map(|(k, v)| format!("{k}: {}", brief(v)))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        }
    }
    match value.get("fields").and_then(|f| f.get("kind")) {
        Some(kind) => variant(kind).unwrap_or_else(|| brief(kind)),
        None => brief(value),
    }
}

/// What the engine does after writing an `effect` record (16.6): the
/// adapter side of an effect. Timers, emissions, standings, consequences
/// and notes have no dispatch beyond their records.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Dispatch {
    Spawn {
        member: String,
        generation: i64,
    },
    Kill {
        member: String,
        pid: i32,
        signal: i32,
    },
    Abort {
        member: String,
        id: i64,
    },
    Attempt {
        member: String,
        id: i64,
    },
    Build {
        member: String,
    },
    AwaitRelease {
        member: String,
    },
}

/// One machine of the run.
#[derive(Clone, Debug)]
pub struct MachineState {
    pub id: String,
    pub member: String,
    pub value: Value,
    pub subscribes: Vec<String>,
    /// `false` after E128.
    pub alive: bool,
}

impl MachineState {
    /// The `Machine` variant.
    pub fn variant(&self) -> &str {
        match &self.value {
            Value::Enum { variant, .. } => variant,
            _ => "",
        }
    }

    /// The payload record's field `name`.
    pub fn field(&self, name: &str) -> Option<&Value> {
        match &self.value {
            Value::Enum { fields, .. } => field(fields.first()?, name),
            _ => None,
        }
    }

    /// The variant name of the payload's `state` field, when it has one.
    pub fn state(&self) -> Option<&str> {
        match self.field("state")? {
            Value::Enum { variant, .. } => Some(variant),
            _ => None,
        }
    }
}

/// The deterministic core (module doc).
pub struct Core<'a, 'p> {
    evaluator: &'a mut Evaluator<'p>,
    machines: Vec<MachineState>,
    next_seq: u64,
    /// `(at, seq of the SetTimer record)` to `(machine, timer id)`.
    timers: BTreeMap<(i64, u64), (usize, i64)>,
    timer_keys: BTreeMap<(usize, i64), (i64, u64)>,
    /// `(member, attempt id)` to the scheduler that asked.
    attempts: BTreeMap<(String, i64), usize>,
    /// Member to the machine awaiting `Built` / `Released`.
    builds: BTreeMap<String, usize>,
    releases: BTreeMap<String, usize>,
    /// `(contract, period_from)` of every consequence fired (16.11).
    fired: BTreeSet<(String, i64)>,
    /// Contract to the `seq` of its latest `standing` record and to the
    /// standing itself.
    standings: BTreeMap<String, (u64, Value)>,
    /// A machine failed (E128).
    failed: bool,
    step: QualName,
}

impl<'a, 'p> Core<'a, 'p> {
    /// The machines of `model` in their initial state, `seq` 1 next.
    pub fn new(evaluator: &'a mut Evaluator<'p>, model: &Model) -> Core<'a, 'p> {
        let machines = model
            .machines
            .iter()
            .map(|m| MachineState {
                id: m.id.clone(),
                member: m.member.clone(),
                value: m.machine.clone(),
                subscribes: m.subscribes.clone(),
                alive: true,
            })
            .collect();
        Core {
            evaluator,
            machines,
            next_seq: 1,
            timers: BTreeMap::new(),
            timer_keys: BTreeMap::new(),
            attempts: BTreeMap::new(),
            builds: BTreeMap::new(),
            releases: BTreeMap::new(),
            fired: BTreeSet::new(),
            standings: BTreeMap::new(),
            failed: false,
            step: QualName::new(CONTROL_MODULE, "step"),
        }
    }

    pub fn machines(&self) -> &[MachineState] {
        &self.machines
    }

    pub fn machine(&self, id: &str) -> Option<usize> {
        self.machines.iter().position(|m| m.id == id)
    }

    /// Machines of `variant` whose `member` is `member`, in id order.
    pub fn machines_of(&self, member: &str, variant: &str) -> Vec<usize> {
        self.machines
            .iter()
            .enumerate()
            .filter(|(_, m)| m.member == member && m.variant() == variant)
            .map(|(i, _)| i)
            .collect()
    }

    /// Every machine of `variant`, in id order.
    pub fn with_variant(&self, variant: &str) -> Vec<usize> {
        self.machines
            .iter()
            .enumerate()
            .filter(|(_, m)| m.variant() == variant)
            .map(|(i, _)| i)
            .collect()
    }

    pub fn next_seq(&self) -> u64 {
        self.next_seq
    }

    /// Continue numbering after a record the engine wrote itself (a
    /// `segment`) or after the journal's last record (resume, replay).
    pub fn set_next_seq(&mut self, seq: u64) {
        self.next_seq = seq;
    }

    /// Whether a machine failed (E128) or a supervisor gave up: exit 1
    /// (16.3).
    pub fn failed(&self) -> bool {
        self.failed
            || self
                .machines
                .iter()
                .any(|m| m.variant() == "Supervise" && m.state() == Some("Failed"))
    }

    /// The latest standing of every contract, by contract (16.3, 16.15).
    pub fn standings(&self) -> Vec<Standing> {
        self.standings
            .iter()
            .map(|(contract, (_, value))| Standing {
                contract: contract.clone(),
                value: value.clone(),
            })
            .collect()
    }

    /// The current standing of every contract at `at` (16.3 `status`): the
    /// published standing where there is one, else the open window's
    /// standing by `std.control.standing_at`, which no attempt has reached
    /// (16.0 item 4: `InsufficientEvidence`, never `Compliant`). Sorted by
    /// contract.
    pub fn current_standings(&mut self, at: i64) -> Vec<Standing> {
        let standing_at = QualName::new(CONTROL_MODULE, "standing_at");
        let mut standings: BTreeMap<String, Value> = self
            .standings
            .iter()
            .map(|(contract, (_, value))| (contract.clone(), value.clone()))
            .collect();
        let unpublished: Vec<(String, Value)> = self
            .machines
            .iter()
            .filter(|m| m.variant() == "Evaluate")
            .filter_map(|m| {
                let contract = text(m.field("contract")?)?;
                let Value::Enum { fields, .. } = &m.value else {
                    return None;
                };
                let state = fields.first()?.clone();
                (!standings.contains_key(&contract)).then_some((contract, state))
            })
            .collect();
        for (contract, state) in unpublished {
            let instant = Value::Quantity(QuantityKind::Duration, at);
            if let Ok(value) = self
                .evaluator
                .call(&standing_at, vec![state, instant.clone(), instant])
            {
                standings.insert(contract, value);
            }
        }
        standings
            .into_iter()
            .map(|(contract, value)| Standing { contract, value })
            .collect()
    }

    /// The instant of the earliest pending timer.
    pub fn next_timer_at(&self) -> Option<i64> {
        self.timers.keys().next().map(|(at, _)| *at)
    }

    /// Remove and return the earliest timer due at or before `until`:
    /// `(at, machine, timer id)`, in `(at, seq)` order (16.4).
    pub fn next_due(&mut self, until: i64) -> Option<(i64, usize, i64)> {
        let key = *self.timers.keys().next().filter(|(at, _)| *at <= until)?;
        let (machine, id) = self.timers.remove(&key)?;
        self.timer_keys.remove(&(machine, id));
        Some((key.0, machine, id))
    }

    /// The scheduler that asked for attempt `id` of `member`, once.
    pub fn take_attempt(&mut self, member: &str, id: i64) -> Option<usize> {
        self.attempts.remove(&(member.to_string(), id))
    }

    pub fn take_build(&mut self, member: &str) -> Option<usize> {
        self.builds.remove(member)
    }

    pub fn take_release(&mut self, member: &str) -> Option<usize> {
        self.releases.remove(member)
    }

    /// A record of the engine's own (`segment`, `status` is not one):
    /// allocates its `seq`.
    pub fn record(
        &mut self,
        kind: &'static str,
        at: i64,
        machine: Option<String>,
        value: Json,
        durable: bool,
        dispatch: Option<Dispatch>,
    ) -> Record {
        let seq = self.next_seq;
        self.next_seq += 1;
        Record {
            seq,
            at,
            kind,
            machine,
            value,
            durable,
            dispatch,
        }
    }

    /// Feed `kind` (a `std.control.EventKind` value, see [`kind`]) at `at`
    /// to `machine` and every machine an emission reaches: the records in
    /// journal order (module doc). A dead machine produces nothing.
    pub fn feed(&mut self, machine: usize, at: i64, kind: Value) -> Vec<Record> {
        let mut out = Vec::new();
        let mut queue = VecDeque::from([(machine, kind)]);
        while let Some((index, kind)) = queue.pop_front() {
            let Some(state) = self.machines.get(index) else {
                continue;
            };
            if !state.alive {
                continue;
            }
            let id = state.id.clone();
            let current = state.value.clone();
            let event = event(at, kind);
            let record = self.record(
                "event",
                at,
                Some(id.clone()),
                value_json(&event),
                false,
                None,
            );
            out.push(record);
            let args = vec![current, event];
            match self.evaluator.call(&self.step, args) {
                Ok(step) => {
                    let (next, effects) = match split_step(step) {
                        Some(parts) => parts,
                        None => {
                            out.push(self.error(at, &id, "step returned no `std.control.Step`"));
                            continue;
                        }
                    };
                    self.machines[index].value = next;
                    for effect in effects {
                        self.effect(index, at, effect, &mut out, &mut queue);
                    }
                }
                Err(error) => {
                    let report = match error {
                        EvalError::Failed { report } => report,
                        EvalError::Limit { limit, running } => {
                            let limit = match limit {
                                Limit::Fuel(n) => format!("the fuel limit of {n} units"),
                                Limit::Nodes(n) => format!("the value-size limit of {n} nodes"),
                                Limit::Depth(n) => format!("the call-depth limit of {n} calls"),
                            };
                            format!("{limit} exceeded in {running}")
                        }
                        EvalError::Unencodable { what } => {
                            format!("{what} cannot be encoded")
                        }
                    };
                    out.push(self.error(at, &id, &report));
                }
            }
        }
        out
    }

    /// The `error` record of E128 for `machine`; the machine stops.
    fn error(&mut self, at: i64, machine: &str, report: &str) -> Record {
        if let Some(index) = self.machine(machine) {
            self.machines[index].alive = false;
        }
        self.failed = true;
        let mut value = BTreeMap::new();
        value.insert("code".to_string(), Json::Str("E128".to_string()));
        value.insert(
            "report".to_string(),
            Json::Str(format!("reducer of {machine} failed: {report}")),
        );
        self.record(
            "error",
            at,
            Some(machine.to_string()),
            Json::Object(value),
            true,
            None,
        )
    }

    /// One effect of `machine`'s step: its `effect` record and what
    /// follows (module doc).
    fn effect(
        &mut self,
        machine: usize,
        at: i64,
        mut effect: Value,
        out: &mut Vec<Record>,
        queue: &mut VecDeque<(usize, Value)>,
    ) {
        let id = self.machines[machine].id.clone();
        let Some((variant, fields)) = enum_parts(&effect) else {
            out.push(self.error(at, &id, "an effect that is no `std.control.Effect`"));
            return;
        };
        let variant = variant.to_string();
        let fields: Vec<Value> = fields.to_vec();
        let arg_text = |i: usize| -> String { fields.get(i).and_then(text).unwrap_or_default() };
        let arg_int = |i: usize| -> i64 { fields.get(i).and_then(int).unwrap_or_default() };
        let mut dispatch = None;
        let mut follow: Vec<(&'static str, Json)> = Vec::new();
        match variant.as_str() {
            "Spawn" => {
                dispatch = Some(Dispatch::Spawn {
                    member: arg_text(0),
                    generation: arg_int(1),
                });
            }
            "Kill" => {
                dispatch = Some(Dispatch::Kill {
                    member: arg_text(0),
                    pid: arg_int(1) as i32,
                    signal: arg_int(2) as i32,
                });
            }
            "Abort" => {
                self.attempts.remove(&(arg_text(0), arg_int(1)));
                dispatch = Some(Dispatch::Abort {
                    member: arg_text(0),
                    id: arg_int(1),
                });
            }
            "Attempt" => {
                self.attempts.insert((arg_text(0), arg_int(1)), machine);
                dispatch = Some(Dispatch::Attempt {
                    member: arg_text(0),
                    id: arg_int(1),
                });
            }
            "SetTimer" => {
                let timer = arg_int(0);
                let instant = fields.get(1).and_then(int).unwrap_or(at);
                if let Some(old) = self.timer_keys.remove(&(machine, timer)) {
                    self.timers.remove(&old);
                }
                let key = (instant, self.next_seq);
                self.timers.insert(key, (machine, timer));
                self.timer_keys.insert((machine, timer), key);
            }
            "CancelTimer" => {
                if let Some(old) = self.timer_keys.remove(&(machine, arg_int(0))) {
                    self.timers.remove(&old);
                }
            }
            "Emit" => {
                let reference = arg_text(0);
                let value = fields.get(1).cloned().unwrap_or(Value::Unit);
                let mut signal = BTreeMap::new();
                signal.insert("ref".to_string(), Json::Str(reference.clone()));
                signal.insert("value".to_string(), value_json(&value));
                follow.push(("signal", Json::Object(signal)));
                for (index, state) in self.machines.iter().enumerate() {
                    if state.alive && state.subscribes.contains(&reference) {
                        queue.push_back((
                            index,
                            kind(
                                "Emitted",
                                vec![Value::Text(reference.clone()), value.clone()],
                            ),
                        ));
                    }
                }
            }
            "Publish" => {
                let seq = self.next_seq + 1;
                if let Some(standing) = fields.first() {
                    let mut standing = standing.clone();
                    set_field(&mut standing, "version", Value::Int(seq as i64));
                    let contract = field(&standing, "contract")
                        .and_then(text)
                        .unwrap_or_default();
                    self.standings.insert(contract, (seq, standing.clone()));
                    follow.push(("standing", value_json(&standing)));
                    effect = Value::Enum {
                        name: QualName::new(CONTROL_MODULE, "Effect"),
                        variant: variant.clone(),
                        fields: vec![standing],
                    };
                }
            }
            "Fire" => {
                if let Some(fired) = fields.first() {
                    let mut fired = fired.clone();
                    let contract = field(&fired, "contract").and_then(text).unwrap_or_default();
                    let standing_seq = self
                        .standings
                        .get(&contract)
                        .map_or(0, |(seq, _)| *seq as i64);
                    set_field(&mut fired, "standing_seq", Value::Int(standing_seq));
                    let period_from = field(&fired, "period_from").and_then(int).unwrap_or(0);
                    if self.fired.insert((contract, period_from)) {
                        follow.push(("consequence", value_json(&fired)));
                    }
                    effect = Value::Enum {
                        name: QualName::new(CONTROL_MODULE, "Effect"),
                        variant: variant.clone(),
                        fields: vec![fired],
                    };
                }
            }
            "Build" => {
                self.builds.insert(arg_text(0), machine);
                dispatch = Some(Dispatch::Build {
                    member: arg_text(0),
                });
            }
            "AwaitRelease" => {
                self.releases.insert(arg_text(0), machine);
                dispatch = Some(Dispatch::AwaitRelease {
                    member: arg_text(0),
                });
            }
            "Note" => {}
            other => {
                let report = format!("unknown effect `{other}`");
                out.push(self.error(at, &id, &report));
                return;
            }
        }
        out.push(self.record(
            "effect",
            at,
            Some(id.clone()),
            value_json(&effect),
            true,
            dispatch,
        ));
        for (kind, value) in follow {
            out.push(self.record(kind, at, Some(id.clone()), value, true, None));
        }
    }
}

/// A `std.control.EventKind` value: `variant` with `fields`.
pub fn kind(variant: &str, fields: Vec<Value>) -> Value {
    Value::Enum {
        name: QualName::new(CONTROL_MODULE, "EventKind"),
        variant: variant.to_string(),
        fields,
    }
}

/// A `std.control.Event` value at `at`.
pub fn event(at: i64, kind: Value) -> Value {
    Value::Record {
        name: QualName::new(CONTROL_MODULE, "Event"),
        fields: vec![
            (
                "at".to_string(),
                Value::Quantity(QuantityKind::Duration, at),
            ),
            ("kind".to_string(), kind),
        ],
    }
}

/// A `std.control.Segment` value (16.5).
pub fn segment(
    start: i64,
    stop: Option<i64>,
    clock: &str,
    generation: i64,
    declared: &[String],
) -> Value {
    Value::Record {
        name: QualName::new(CONTROL_MODULE, "Segment"),
        fields: vec![
            (
                "start".to_string(),
                Value::Quantity(QuantityKind::Duration, start),
            ),
            (
                "stop".to_string(),
                Value::option(stop.map(|s| Value::Quantity(QuantityKind::Duration, s))),
            ),
            ("clock".to_string(), Value::Text(clock.to_string())),
            ("generation".to_string(), Value::Int(generation)),
            (
                "declared".to_string(),
                Value::List(declared.iter().map(|d| Value::Text(d.clone())).collect()),
            ),
        ],
    }
}

/// The `{"ns": n, "type": "Duration"}` of 15.4.
pub fn duration_json(ns: i64) -> Json {
    let mut object = BTreeMap::new();
    object.insert("ns".to_string(), Json::Int(ns));
    object.insert("type".to_string(), Json::Str("Duration".to_string()));
    Json::Object(object)
}

/// The variant and payload of an enum value.
pub fn enum_parts(value: &Value) -> Option<(&str, &[Value])> {
    match value {
        Value::Enum {
            variant, fields, ..
        } => Some((variant, fields)),
        _ => None,
    }
}

/// Field `name` of a record value.
pub fn field<'v>(record: &'v Value, name: &str) -> Option<&'v Value> {
    match record {
        Value::Record { fields, .. } => fields.iter().find(|(f, _)| f == name).map(|(_, v)| v),
        _ => None,
    }
}

fn set_field(record: &mut Value, name: &str, value: Value) {
    if let Value::Record { fields, .. } = record
        && let Some((_, slot)) = fields.iter_mut().find(|(f, _)| f == name)
    {
        *slot = value;
    }
}

pub fn text(value: &Value) -> Option<String> {
    match value {
        Value::Text(text) => Some(text.clone()),
        _ => None,
    }
}

/// An `Int` or a quantity in base units.
pub fn int(value: &Value) -> Option<i64> {
    match value {
        Value::Int(n) | Value::Quantity(_, n) => Some(*n),
        _ => None,
    }
}

/// The machine and the effects of a `std.control.Step` value.
fn split_step(step: Value) -> Option<(Value, Vec<Value>)> {
    let Value::Record { fields, .. } = step else {
        return None;
    };
    let mut machine = None;
    let mut effects = None;
    for (name, value) in fields {
        match name.as_str() {
            "machine" => machine = Some(value),
            "effects" => {
                effects = match value {
                    Value::List(items) => Some(items),
                    _ => None,
                }
            }
            _ => {}
        }
    }
    Some((machine?, effects?))
}
