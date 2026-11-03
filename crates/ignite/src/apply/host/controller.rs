//! Driving admitted revisions: the bridge to the run driver, the records it
//! sends back, ownership and readiness of the owned instances, the
//! transition from the running revision to the desired one, and the
//! completion or rejection of an operation.
//!
//! Transitions (`docs/slice.md` 17.4, decision D1): the diff of the base and
//! desired revisions is the plan's `actions` (`ignite-apply-actions/0.1`);
//! what happens in which order is the pure `std.control.transit` reducer,
//! stepped here in the evaluator over the desired revision's retained
//! standard library. Every step is a host journal record of kind
//! `transition` (`operationId`, `event`, `effects`, `transition`), written
//! before its effects are executed. The effects reach the running revision
//! as control lines (`run::live`): `StopMember` retires the member's machines
//! and drains its processes, `SwitchRevision` hands the instances that stay
//! to the desired revision (the same run when the semantic model and the
//! reducer revision are equal, else a handover to a new run, or a first run),
//! `StartMember` starts a deferred member's machines, `CutOver` rolls a member
//! through its `DrainRollout`, `Commit` completes the operation and `Abandon`
//! fails it with its reason (`gave_up`, `ordering_cycle`).
//!
//! The driver reports back: `MemberStopped` once the member has no live owned
//! process, `MemberReady` once every desired machine of the member reports
//! ready (a supervisor's accepted `Ready`, a scheduler's first `signal`, an
//! evaluator's first `standing`), `MemberGaveUp` at a supervisor's
//! `Note("gave_up")`, `Converged` once every desired machine reports ready and
//! no retiring instance is left. An effect the driver cannot carry out is
//! the reducer event `EffectFailed(reason, member)`, whose `Abandon` is
//! journaled like every step before the operation fails: a replacement's
//! listener that cannot be bound, a process that failed to spawn, a ready
//! process whose artifact is not the admitted one, a run that ended.
//!
//! A cutover whose new instance listens on the same address and port as
//! the old one drains the old listener first and then starts the
//! replacement, which must become ready; a failure after the drain leaves
//! the operation `recovery_required` with the state as it is. Any other
//! cutover (another port, or no listener) starts the new instance first
//! (`rollout <member> start_first`) and drains the old one only once the
//! new one is ready, so a replacement that never becomes ready leaves the
//! old instance serving. Before the first step the host binds every such
//! new listener once and fails the operation with `listener_unavailable`
//! when it cannot; like `gave_up`, that leaves the committed revision's
//! instances as they were, and a corrective plan resolves it.
//!
//! A start-first replacement that fails after it was spawned leaves the old
//! instance serving, but in the desired revision's run, whose supervisor
//! gave the member up. When such a transition is abandoned, the old instance
//! is the member's again (`restore <member>`), and every live owned instance
//! another operation started is handed to a new run of the committed plan
//! (journal record `restoreCommitted`): its supervisors adopt them with the
//! committed policy, generation, artifact and port and their readiness, and
//! what the failed operation started stops at that handover. An instance
//! keeps the operation that started it across handovers.
//!
//! A corrective plan back to the committed revision, applied while the
//! running revision is still the failed operation's, is a transition too:
//! its member actions are the diff from the failed plan to the corrective
//! one, so it stops what the failed operation started and starts what it
//! stopped, each step journaled; a member whose live instance another
//! operation started is kept.
//!
//! The transition is differential when the running revision realizes the
//! committed one, the plan's base. Otherwise (no running revision, or one
//! that realizes an operation that did not commit) every owned desired member
//! starts afresh and the running revision's instances are stopped at the
//! switch.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, atomic::Ordering, mpsc};

use ignite_eval::{CONTROL_MODULE, Evaluator, Fuel, Json, Value, value_json};
use ignite_types::QualName;

use super::admission::retained_plan;
use super::journal::{FORMAT, save_json};
use super::{Host, Message, read_json};
use crate::apply::revision::Launch;
use crate::apply::{Failure, fail, object, plan::Plan, text};
use crate::run::control::Observation;
use crate::run::engine::{enum_parts, field};
use crate::run::live::Carried;

/// The revision controller running on the run driver's thread.
pub(super) struct Runtime {
    pub(super) operation: String,
    pub(super) controller_plan: String,
    /// The plan whose desired state this run realizes: the controller plan,
    /// or the plan a transition switched it to.
    pub(super) desired: String,
    /// The semantic model and reducer revision of the controller plan.
    pub(super) model: Json,
    pub(super) reducer: String,
    pub(super) operation_sink: Arc<Mutex<String>>,
    pub(super) artifacts: Arc<Mutex<BTreeMap<String, PathBuf>>>,
    /// Member to the lowest generation whose `Ready` counts during a cutover.
    pub(super) minimum_generation: BTreeMap<String, i64>,
    pub(super) control: mpsc::Sender<Observation>,
    pub(super) ready: BTreeSet<String>,
    pub(super) expected: BTreeSet<String>,
    /// Pids a previous revision's run handed over: their artifact was proven
    /// when they became ready.
    pub(super) adopted: BTreeSet<i64>,
    /// The supervisor machines: their lifecycle events carry ownership.
    pub(super) supervised: BTreeSet<String>,
    /// The committed plan whose run takes this run's handover: set when a
    /// failed start-first cutover gives the old instances back (module doc).
    pub(super) restore_to: Option<String>,
}

/// The transition of one operation in flight.
pub(super) struct Driving {
    operation: String,
    plan: Plan,
    loaded: crate::Loaded,
    value: Value,
    phase: String,
    queue: VecDeque<Value>,
    stopping: Option<String>,
    starting: Option<String>,
    /// Waiting for the desired revision's run to initialize.
    switching: bool,
    /// The members the desired revision's run starts on `start <member>`.
    deferred: BTreeSet<String>,
    /// The members whose instances stay at the switch.
    keep: BTreeSet<String>,
    /// The cutover members whose new instance starts before the old drains.
    start_first: BTreeSet<String>,
    /// Member to the pid a start-first replacement displaced, restored as
    /// the owned instance when the replacement ends before it was ready.
    displaced: BTreeMap<String, i64>,
    /// Member to the plan's action kind.
    kinds: BTreeMap<String, String>,
    /// The operation the running revision carried before this transition
    /// took it over; it carries it again when the transition is abandoned
    /// before the run realizes the desired plan.
    base_operation: Option<String>,
}

/// Whether an operation failed with `reason` left the committed revision's
/// instances as they were, so that a corrective plan resolves it (D3).
pub(super) fn resolvable(reason: &str) -> bool {
    matches!(reason, "gave_up" | "listener_unavailable")
}

const NEXT_CORRECTIVE: &str = "apply a corrective plan prepared against the committed revision";
const NEXT_INSPECT: &str = "inspect retained ownership and journal before recovery";

/// The address and declared port of the listener `member` provisions in
/// `model`.
fn listener_of(model: &Json, member: &str) -> Option<(String, i64)> {
    let resource = model
        .get("edges")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
        .find(|edge| {
            edge.get("kind").and_then(Json::as_str) == Some("provisioning")
                && edge.get("to").and_then(Json::as_str) == Some(member)
        })?
        .get("from")
        .and_then(Json::as_str)?;
    let properties = declared(model, resource)?;
    Some((
        properties
            .get("address")
            .and_then(Json::as_str)
            .unwrap_or("127.0.0.1")
            .to_string(),
        properties.get("port").and_then(Json::as_i64).unwrap_or(0),
    ))
}

/// The properties of resource `id` in `model`.
fn declared<'m>(model: &'m Json, id: &str) -> Option<&'m Json> {
    model
        .get("resources")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
        .find(|resource| resource.get("id").and_then(Json::as_str) == Some(id))?
        .get("properties")
}

/// One member action as the reducer receives it.
struct Action {
    member: String,
    kind: String,
    ownership: String,
    cutover: bool,
    depends_on: Vec<String>,
}

impl Action {
    fn starts(&self) -> bool {
        self.ownership == "owned"
            && (self.kind == "add" || (self.kind == "replace" && !self.cutover))
    }
}

/// Whether the process an ownership record names is still that process.
fn instance_alive(owner: &Json) -> bool {
    owner
        .get("pid")
        .and_then(Json::as_i64)
        .zip(owner.get("instance").and_then(Json::as_str))
        .is_some_and(|(pid, instance)| process_identity(pid).as_deref() == Some(instance))
}

/// Whether a pinned `port` of `resource` stays for a run of `model`: the
/// resource declares no other fixed port.
fn keeps_pin(model: &Json, resource: &str, port: u16) -> bool {
    declared(model, resource)
        .and_then(|properties| properties.get("port"))
        .and_then(Json::as_i64)
        .is_none_or(|declared| declared == 0 || declared == i64::from(port))
}

/// The start time of a live process, `None` when it is gone or a zombie.
pub(super) fn process_identity(pid: i64) -> Option<String> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let fields = stat.rsplit_once(") ")?.1;
    if matches!(fields.split_whitespace().next(), Some("Z" | "X")) {
        return None;
    }
    Some(fields.split_whitespace().nth(19)?.to_string())
}

fn texts(value: Option<&Json>) -> Vec<String> {
    value
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
        .filter_map(Json::as_str)
        .map(str::to_string)
        .collect()
}

fn text_list(items: &[String]) -> Value {
    Value::List(items.iter().cloned().map(Value::Text).collect())
}

fn transition_event(variant: &str, member: Option<&str>) -> Value {
    Value::Enum {
        name: QualName::new(CONTROL_MODULE, "TransitionEvent"),
        variant: variant.to_string(),
        fields: member
            .map(|m| vec![Value::Text(m.to_string())])
            .unwrap_or_default(),
    }
}

fn effect_failed_event(reason: &str, member: &str) -> Value {
    Value::Enum {
        name: QualName::new(CONTROL_MODULE, "TransitionEvent"),
        variant: "EffectFailed".to_string(),
        fields: vec![
            Value::Text(reason.to_string()),
            Value::Text(member.to_string()),
        ],
    }
}

/// The member of a machine id `<member>.<role>`.
fn member_of(machine: &str) -> &str {
    machine
        .rsplit_once('.')
        .map_or(machine, |(member, _)| member)
}

/// The machines of `model` whose readiness the host awaits.
fn awaited(model: &ignite_eval::Model) -> BTreeSet<String> {
    model
        .machines
        .iter()
        .filter(|machine| matches!(machine.variant(), "Supervise" | "Schedule" | "Evaluate"))
        .map(|machine| machine.id.clone())
        .collect()
}

/// `members` with every provider before its dependents, keeping the order
/// of `priority` otherwise; a member left in a cycle keeps its place last.
fn provider_first(members: Vec<String>, depends: &BTreeMap<String, Vec<String>>) -> Vec<String> {
    let set: BTreeSet<&String> = members.iter().collect();
    let mut placed: Vec<String> = Vec::new();
    let mut left = members.clone();
    while !left.is_empty() {
        let next = left.iter().position(|member| {
            depends.get(member).into_iter().flatten().all(|provider| {
                !set.contains(provider) || placed.contains(provider) || provider == member
            })
        });
        match next {
            Some(index) => placed.push(left.remove(index)),
            None => {
                placed.append(&mut left);
            }
        }
    }
    placed
}

impl Host {
    pub(super) fn complete(&mut self, id: &str) -> Result<(), Failure> {
        self.revision += 1;
        self.committed = Some(id.to_string());
        self.contexts.remove(id);
        self.active = None;
        self.recovery_reason = None;
        if let Some(runtime) = &mut self.runtime {
            runtime.desired = id.to_string();
        }
        let receipt = self
            .receipts
            .get_mut(id)
            .and_then(|receipt| match receipt {
                Json::Object(fields) => Some(fields),
                _ => None,
            })
            .unwrap();
        receipt.insert("state".into(), text("completed"));
        receipt.insert("revision".into(), Json::Int(self.revision));
        self.persist()?;
        let receipt = self.receipt(id);
        for waiter in self.waiters.remove(id).unwrap_or_default() {
            let _ = waiter.send(Ok(receipt.clone()));
        }
        Ok(())
    }

    /// Fail operation `id` with `reason`. While an unresolved recovery stands
    /// ([`Host::standing`]) the first reason recorded stays the reported one
    /// (17.5): `reason` is kept beside it ([`Host::keep_beside`]) and
    /// neither the active operation nor the recovery reason changes.
    pub(super) fn reject_operation(&mut self, id: &str, reason: &str) -> Result<(), Failure> {
        if self
            .transition
            .as_ref()
            .is_some_and(|driving| driving.operation == id)
        {
            self.transition = None;
        }
        if let Some(standing) = self.standing() {
            self.keep_beside(&standing, id, reason);
            self.persist()?;
            for waiter in self.waiters.remove(id).unwrap_or_default() {
                let _ = waiter.send(Err(fail(1, reason)));
            }
            return Ok(());
        }
        if let Some(Json::Object(receipt)) = self.receipts.get_mut(id)
            && receipt.get("state").and_then(Json::as_str) != Some("completed")
        {
            receipt.insert("state".into(), text("recovery_required"));
            receipt.insert("reason".into(), text(reason));
            receipt.insert(
                "nextAction".into(),
                text(if resolvable(reason) {
                    NEXT_CORRECTIVE
                } else {
                    NEXT_INSPECT
                }),
            );
        }
        self.active = Some(id.to_string());
        self.recovery_reason = Some(reason.to_string());
        self.persist()?;
        for waiter in self.waiters.remove(id).unwrap_or_default() {
            let _ = waiter.send(Err(fail(1, reason)));
        }
        Ok(())
    }

    /// The active operation whose unresolved recovery stands, if one does
    /// ([`Host::unresolved`]).
    pub(super) fn standing(&self) -> Option<String> {
        self.active.clone().filter(|active| self.unresolved(active))
    }

    /// Keep `reason`, recorded later for operation `id`, in the receipt of
    /// the `standing` operation as an entry of `laterReasons`, beside the
    /// reason it reports and without replacing it (17.5). A later reason a
    /// corrective plan does not resolve makes the next action an inspection
    /// and the recovery no longer correctable ([`Host::correctable`]). The
    /// committed revision's own recovery keeps nothing beside it: its
    /// receipt stays `completed`, and its reason is never resolvable.
    pub(super) fn keep_beside(&mut self, standing: &str, id: &str, reason: &str) {
        let Some(Json::Object(receipt)) = self.receipts.get_mut(standing) else {
            return;
        };
        if receipt.get("state").and_then(Json::as_str) != Some("recovery_required") {
            return;
        }
        let entry = object([("operationId", text(id)), ("reason", text(reason))]);
        match receipt
            .entry("laterReasons".to_string())
            .or_insert_with(|| Json::Array(Vec::new()))
        {
            Json::Array(later) => later.push(entry),
            other => *other = Json::Array(vec![entry]),
        }
        if !resolvable(reason) {
            receipt.insert("nextAction".into(), text(NEXT_INSPECT));
        }
    }

    /// Whether a corrective plan resolves the host's recovery (D3): its
    /// reason is resolvable and the active operation's receipt keeps no
    /// later reason beside it that is not.
    pub(super) fn correctable(&self) -> bool {
        let later_unresolvable = self
            .active
            .as_ref()
            .and_then(|id| self.receipts.get(id))
            .and_then(|receipt| receipt.get("laterReasons"))
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
            .any(|later| {
                later
                    .get("reason")
                    .and_then(Json::as_str)
                    .is_none_or(|reason| !resolvable(reason))
            });
        self.recovery_reason.as_deref().is_some_and(resolvable) && !later_unresolvable
    }

    /// Hand an admitted, journaled operation to the transition driver: build
    /// the reducer's `Transition` from the plan's member actions and step it
    /// from `Begin` (module doc).
    pub(super) fn dispatch(&mut self, plan: Plan) -> Result<(), Failure> {
        self.dispatch_from(plan, None)
    }

    /// `dispatch` of a corrective plan back to the committed revision while
    /// the running revision still realizes `failed`, the operation it
    /// resolves: the member actions are the diff from `failed` to the plan,
    /// so the transition stops what `failed` started and starts what it
    /// stopped. A member whose live instance another operation started keeps
    /// it (module doc).
    pub(super) fn revert(&mut self, plan: Plan, failed: &str) -> Result<(), Failure> {
        self.dispatch_from(plan, Some(failed))
    }

    fn dispatch_from(&mut self, plan: Plan, failed: Option<&str>) -> Result<(), Failure> {
        let frozen = match self.contexts.get(&plan.id) {
            Some(context) => context.plan.clone(),
            None => retained_plan(&self.root, &plan)?,
        };
        let loaded = crate::load_for(&frozen.source, frozen.std.as_deref(), crate::RUN_ROOTS)?;
        let plan_actions = match failed {
            Some(failed) => {
                let value = read_json(&self.root.join("plans").join(format!("{failed}.json")))?;
                let base = Plan::parse(&value).map_err(|reason| fail(3, reason))?;
                let mut back = plan.clone();
                back.annotate(Some(&base), &Json::Object(self.owned.clone()))
                    .map_err(|reason| fail(2, format!("corrective plan: {reason}")))?;
                back.actions
            }
            None => plan.actions.clone(),
        };
        let differential = failed.is_some()
            || self.committed.is_some()
                && self
                    .runtime
                    .as_ref()
                    .is_some_and(|runtime| Some(&runtime.desired) == self.committed.as_ref());
        let base_rollouts: BTreeSet<String> = self
            .runtime
            .as_ref()
            .and_then(|runtime| runtime.model.get("machines"))
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
            .filter(|machine| {
                machine
                    .get("machine")
                    .and_then(|m| m.get("variant"))
                    .and_then(Json::as_str)
                    == Some("Roll")
            })
            .filter_map(|machine| machine.get("member").and_then(Json::as_str))
            .map(str::to_string)
            .collect();
        let desired_rollouts: BTreeSet<String> = crate::apply::plan::semantic_model(&frozen.model)
            .get("machines")
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
            .filter(|machine| {
                machine
                    .get("machine")
                    .and_then(|m| m.get("variant"))
                    .and_then(Json::as_str)
                    == Some("Roll")
            })
            .filter_map(|machine| machine.get("member").and_then(Json::as_str))
            .map(str::to_string)
            .collect();
        let mut actions = Vec::new();
        let mut kinds = BTreeMap::new();
        for entry in plan_actions
            .get("members")
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
        {
            let member = entry.get("id").and_then(Json::as_str).unwrap_or_default();
            let mut kind = entry.get("kind").and_then(Json::as_str).unwrap_or_default();
            if kind == "replace"
                && let Some(failed) = failed
                && self.owned.get(member).is_some_and(|owner| {
                    owner.get("operationId").and_then(Json::as_str) != Some(failed)
                        && instance_alive(owner)
                })
            {
                kind = "unchanged";
            }
            kinds.insert(member.to_string(), kind.to_string());
            let ownership = entry
                .get("ownership")
                .and_then(Json::as_str)
                .unwrap_or_default();
            let depends_on = texts(entry.get("dependsOn"));
            let (kind, cutover) = if differential {
                let live = self.owned.get(member).is_some_and(|owner| {
                    owner.get("ready").and_then(Json::as_bool) == Some(true)
                        && owner
                            .get("pid")
                            .and_then(Json::as_i64)
                            .zip(owner.get("instance").and_then(Json::as_str))
                            .is_some_and(|(pid, instance)| {
                                process_identity(pid).as_deref() == Some(instance)
                            })
                });
                let cutover = kind == "replace"
                    && live
                    && base_rollouts.contains(member)
                    && desired_rollouts.contains(member);
                (kind.to_string(), cutover)
            } else if kind == "remove" {
                continue;
            } else {
                ("add".to_string(), false)
            };
            actions.push(Action {
                member: member.to_string(),
                kind,
                ownership: ownership.to_string(),
                cutover,
                depends_on,
            });
        }
        let depends: BTreeMap<String, Vec<String>> = actions
            .iter()
            .map(|a| (a.member.clone(), a.depends_on.clone()))
            .collect();
        let (start_order, stop_order) = if differential {
            (
                texts(plan_actions.get("startOrder")),
                texts(plan_actions.get("stopOrder")),
            )
        } else {
            let mut order = texts(plan_actions.get("startOrder"));
            order.retain(|member| actions.iter().any(|a| &a.member == member && a.starts()));
            for action in &actions {
                if action.starts() && !order.contains(&action.member) {
                    order.push(action.member.clone());
                }
            }
            (provider_first(order, &depends), Vec::new())
        };
        let deferred: BTreeSet<String> = actions
            .iter()
            .filter(|a| a.starts())
            .map(|a| a.member.clone())
            .collect();
        let keep: BTreeSet<String> = if differential {
            actions
                .iter()
                .filter(|a| a.kind != "remove" && !deferred.contains(&a.member))
                .filter(|a| !(a.kind == "replace" && !a.cutover))
                .map(|a| a.member.clone())
                .collect()
        } else {
            BTreeSet::new()
        };
        // A cutover keeps its old listener only when the new instance would
        // listen on the same one (module doc); the new listener of every
        // other cutover must be bindable before anything is stopped.
        let mut start_first = BTreeSet::new();
        let mut unavailable = None;
        let base_model = self.runtime.as_ref().map(|runtime| &runtime.model);
        for action in actions.iter().filter(|a| a.cutover) {
            let old = base_model.and_then(|model| listener_of(model, &action.member));
            let new = listener_of(&frozen.model, &action.member);
            if old.is_some() && old == new {
                continue;
            }
            start_first.insert(action.member.clone());
            if unavailable.is_none()
                && let Some((address, port)) = &new
                && let Ok(port) = u16::try_from(*port)
                && port > 0
                && std::net::TcpListener::bind((address.as_str(), port)).is_err()
            {
                unavailable = Some(action.member.clone());
            }
        }
        let value = {
            let modules = loaded.modules();
            let mut evaluator = Evaluator::new(&modules, Fuel::default());
            let mut values = Vec::new();
            for action in &actions {
                values.push(
                    evaluator
                        .call(
                            &QualName::new(CONTROL_MODULE, "member_action"),
                            vec![
                                Value::Text(action.member.clone()),
                                Value::Text(action.kind.clone()),
                                Value::Text(action.ownership.clone()),
                                Value::Bool(action.cutover),
                                text_list(&action.depends_on),
                            ],
                        )
                        .map_err(|error| fail(2, format!("transition: {error:?}")))?,
                );
            }
            evaluator
                .call(
                    &QualName::new(CONTROL_MODULE, "transition"),
                    vec![
                        Value::Text(plan.id.clone()),
                        Value::List(values),
                        text_list(&start_order),
                        text_list(&stop_order),
                    ],
                )
                .map_err(|error| fail(2, format!("transition: {error:?}")))?
        };
        let base_operation = self
            .runtime
            .as_ref()
            .map(|runtime| runtime.operation.clone());
        self.carry_operation(&plan.id)?;
        self.transition = Some(Driving {
            operation: plan.id.clone(),
            plan: frozen,
            loaded,
            value,
            phase: "Planned".into(),
            queue: VecDeque::from([match &unavailable {
                Some(member) => effect_failed_event("listener_unavailable", member),
                None => transition_event("Begin", None),
            }]),
            stopping: None,
            starting: None,
            switching: false,
            deferred,
            keep,
            start_first,
            displaced: BTreeMap::new(),
            kinds,
            base_operation,
        });
        self.drive()
    }

    /// The transition of `operation` is abandoned. A running revision that
    /// does not realize its plan (the run never switched to it, and no
    /// restoration hands the run over to the committed plan) carries the
    /// operation it carried before again: its records and `runtimeOperation`
    /// no longer name the failed operation.
    fn release_operation(&mut self, operation: &str) -> Result<(), Failure> {
        let base = self
            .transition
            .as_ref()
            .and_then(|driving| driving.base_operation.clone());
        let realizes_base = self
            .runtime
            .as_ref()
            .is_some_and(|runtime| runtime.desired != operation && runtime.restore_to.is_none());
        match base {
            Some(base) if realizes_base => self.carry_operation(&base),
            _ => Ok(()),
        }
    }

    /// The running revision's records and `runtimeOperation` carry
    /// operation `id` from here on.
    fn carry_operation(&mut self, id: &str) -> Result<(), Failure> {
        if let Some(runtime) = &mut self.runtime {
            runtime.operation = id.to_string();
            *runtime
                .operation_sink
                .lock()
                .map_err(|_| fail(3, "operation identity lock failed"))? = id.to_string();
        }
        Ok(())
    }

    /// Whether `member` has no live owned process.
    fn member_stopped(&self, member: &str) -> bool {
        self.owned.get(member).is_none_or(|owner| {
            !owner
                .get("pid")
                .and_then(Json::as_i64)
                .zip(owner.get("instance").and_then(Json::as_str))
                .is_some_and(|(pid, instance)| process_identity(pid).as_deref() == Some(instance))
        })
    }

    /// Whether every awaited machine of `member` in the running revision
    /// reports ready.
    fn member_ready(&self, member: &str) -> bool {
        self.runtime.as_ref().is_some_and(|runtime| {
            runtime
                .expected
                .iter()
                .filter(|machine| member_of(machine) == member)
                .all(|machine| runtime.ready.contains(machine))
        })
    }

    fn converged(&self) -> bool {
        self.retiring.is_empty()
            && self
                .runtime
                .as_ref()
                .is_some_and(|runtime| runtime.expected.is_subset(&runtime.ready))
    }

    /// Step the transition over every event that is due until none is.
    pub(super) fn drive(&mut self) -> Result<(), Failure> {
        loop {
            let Some(driving) = &self.transition else {
                return Ok(());
            };
            let stopped = driving
                .stopping
                .as_deref()
                .filter(|member| self.member_stopped(member))
                .map(str::to_string);
            let ready = driving
                .starting
                .as_deref()
                .filter(|member| !driving.switching && self.member_ready(member))
                .map(str::to_string);
            let converged = driving.phase == "Converging" && !driving.switching && self.converged();
            let driving = self.transition.as_mut().unwrap();
            let event = if let Some(event) = driving.queue.pop_front() {
                event
            } else if let Some(member) = stopped {
                driving.stopping = None;
                transition_event("MemberStopped", Some(&member))
            } else if let Some(member) = ready {
                driving.starting = None;
                transition_event("MemberReady", Some(&member))
            } else if converged {
                transition_event("Converged", None)
            } else {
                return Ok(());
            };
            self.transit(event)?;
        }
    }

    /// One reducer step: journal it, then execute its effects in order.
    fn transit(&mut self, event: Value) -> Result<(), Failure> {
        let driving = self.transition.as_mut().unwrap();
        let operation = driving.operation.clone();
        let stepped = {
            let modules = driving.loaded.modules();
            let mut evaluator = Evaluator::new(&modules, Fuel::default());
            evaluator.call(
                &QualName::new(CONTROL_MODULE, "transit"),
                vec![driving.value.clone(), event.clone()],
            )
        };
        let step = match stepped {
            Ok(step) => step,
            Err(error) => {
                self.append(
                    "transition",
                    object([
                        ("operationId", text(&operation)),
                        ("event", value_json(&event)),
                        ("error", text(format!("{error:?}"))),
                    ]),
                )?;
                return self.reject_operation(&operation, "transition reducer failed");
            }
        };
        let next = field(&step, "transition").cloned().unwrap_or(Value::Unit);
        let effects = match field(&step, "effects") {
            Some(Value::List(effects)) => effects.clone(),
            _ => Vec::new(),
        };
        self.append(
            "transition",
            object([
                ("operationId", text(&operation)),
                ("event", value_json(&event)),
                (
                    "effects",
                    Json::Array(effects.iter().map(value_json).collect()),
                ),
                ("transition", value_json(&next)),
            ]),
        )?;
        let driving = self.transition.as_mut().unwrap();
        driving.phase = field(&next, "phase")
            .and_then(enum_parts)
            .map(|(variant, _)| variant.to_string())
            .unwrap_or_default();
        driving.value = next;
        for effect in effects {
            let Some((variant, fields)) = enum_parts(&effect) else {
                continue;
            };
            let first = fields.first().and_then(crate::run::engine::text);
            match (variant, first) {
                ("StopMember", Some(member)) => self.stop_member(&member)?,
                ("SwitchRevision", _) => self.switch()?,
                ("StartMember", Some(member)) => self.start_member(&member, false)?,
                ("CutOver", Some(member)) => self.start_member(&member, true)?,
                ("Commit", _) => {
                    self.transition = None;
                    if self.active.as_deref() == Some(operation.as_str()) {
                        self.complete(&operation)?;
                    }
                }
                ("Abandon", Some(reason)) => {
                    self.restore_displaced(&operation)?;
                    self.release_operation(&operation)?;
                    return self.reject_operation(&operation, &reason);
                }
                _ => {}
            }
            if self.transition.is_none() {
                break;
            }
        }
        Ok(())
    }

    /// The transition of `operation` is abandoned: every start-first cutover
    /// whose replacement never became ready gives its displaced instance
    /// back, and the instances of the committed revision (every live owned
    /// one another operation started) are handed to a run of the committed
    /// plan, whose supervisors adopt them as they were before the attempt
    /// (module doc). Nothing happens without such a cutover.
    fn restore_displaced(&mut self, operation: &str) -> Result<(), Failure> {
        let (Some(driving), Some(committed)) = (&self.transition, self.committed.clone()) else {
            return Ok(());
        };
        if self.runtime.is_none() {
            return Ok(());
        }
        let mut restored = Vec::new();
        for (member, old) in driving.displaced.clone() {
            let record = self.retiring.get(&old.to_string()).cloned().or_else(|| {
                self.owned
                    .get(&member)
                    .filter(|owner| owner.get("pid").and_then(Json::as_i64) == Some(old))
                    .cloned()
            });
            let Some(record) = record.filter(instance_alive) else {
                continue;
            };
            let replaced = self.owned.get(&member).is_some_and(|owner| {
                owner.get("pid").and_then(Json::as_i64) != Some(old)
                    && owner.get("ready").and_then(Json::as_bool) == Some(true)
            });
            if replaced {
                continue;
            }
            self.retiring.remove(&old.to_string());
            self.owned.insert(member.clone(), record);
            restored.push(member);
        }
        if restored.is_empty() {
            return Ok(());
        }
        let keep: Vec<String> = self
            .owned
            .iter()
            .filter(|(_, owner)| {
                owner.get("operationId").and_then(Json::as_str) != Some(operation)
                    && instance_alive(owner)
            })
            .map(|(member, _)| member.clone())
            .collect();
        self.append(
            "restoreCommitted",
            object([
                ("operationId", text(operation)),
                ("committedPlanId", text(&committed)),
                ("restored", Json::Array(restored.iter().map(text).collect())),
                ("keep", Json::Array(keep.iter().map(text).collect())),
            ]),
        )?;
        self.send(format!("restore {}", restored.join(" ")))?;
        self.send(format!("handover {}", keep.join(" ")))?;
        if let Some(runtime) = &mut self.runtime {
            runtime.restore_to = Some(committed);
        }
        Ok(())
    }

    fn send(&self, line: String) -> Result<(), Failure> {
        if let Some(runtime) = &self.runtime {
            runtime
                .control
                .send(Observation::Control(line))
                .map_err(|_| fail(1, "revision controller is unavailable"))?;
        }
        Ok(())
    }

    fn forget_ready(&mut self, member: &str) {
        if let Some(runtime) = &mut self.runtime {
            runtime.ready.retain(|machine| member_of(machine) != member);
        }
    }

    fn stop_member(&mut self, member: &str) -> Result<(), Failure> {
        self.forget_ready(member);
        if let Some(driving) = &mut self.transition {
            driving.stopping = Some(member.to_string());
        }
        self.send(format!("retire {member}"))
    }

    fn start_member(&mut self, member: &str, cutover: bool) -> Result<(), Failure> {
        if let Some(driving) = &mut self.transition {
            driving.starting = Some(member.to_string());
        }
        if cutover {
            let minimum = self
                .owned
                .get(member)
                .and_then(|owner| owner.get("generation"))
                .and_then(Json::as_i64)
                .unwrap_or(1)
                + 1;
            if let Some(runtime) = &mut self.runtime {
                runtime
                    .minimum_generation
                    .insert(member.to_string(), minimum);
            }
            self.forget_ready(member);
            let start_first = self
                .transition
                .as_ref()
                .is_some_and(|driving| driving.start_first.contains(member));
            self.send(if start_first {
                format!("rollout {member} start_first")
            } else {
                format!("rollout {member}")
            })
        } else {
            self.send(format!("start {member}"))
        }
    }

    /// `SwitchRevision`: rebind the running revision to the desired plan's
    /// artifacts when both have the same semantic model and reducer, hand its
    /// staying instances to a new run otherwise, or start the first run.
    fn switch(&mut self) -> Result<(), Failure> {
        let driving = self.transition.as_mut().unwrap();
        let plan = driving.plan.clone();
        let Some(runtime) = &mut self.runtime else {
            driving.switching = true;
            let deferred = driving.deferred.clone();
            return self.start_runtime(
                plan,
                Some(Launch {
                    carried: None,
                    deferred,
                }),
            );
        };
        let same = self.committed.as_ref() == Some(&runtime.desired)
            && runtime.model == crate::apply::plan::semantic_model(&plan.model)
            && runtime.reducer == plan.reducer_revision;
        if same {
            let artifacts: BTreeMap<_, _> = plan
                .artifacts
                .iter()
                .map(|(member, artifact)| (member.clone(), artifact.path.clone()))
                .collect();
            *runtime
                .artifacts
                .lock()
                .map_err(|_| fail(3, "artifact manifest lock failed"))? = artifacts;
            runtime.desired = plan.id.clone();
            driving.queue.push_back(transition_event("Switched", None));
            self.send("rebind".into())
        } else {
            driving.switching = true;
            let keep: Vec<String> = driving.keep.iter().cloned().collect();
            self.send(format!("handover {}", keep.join(" ")))
        }
    }

    /// The running revision handed its staying instances over: start the
    /// desired revision's run with them.
    pub(super) fn handed_over(&mut self, id: &str, carried: Carried) -> Result<(), Failure> {
        if !self
            .runtime
            .as_ref()
            .is_some_and(|runtime| runtime.controller_plan == id)
        {
            return Ok(());
        }
        let restore_to = self
            .runtime
            .as_ref()
            .and_then(|runtime| runtime.restore_to.clone());
        let mut carried = carried;
        let Some(driving) = &self.transition else {
            self.runtime = None;
            let Some(committed) = restore_to else {
                drop(carried);
                return Ok(());
            };
            let value = read_json(&self.root.join("plans").join(format!("{committed}.json")))?;
            let plan = Plan::parse(&value).map_err(|reason| fail(3, reason))?;
            carried
                .pinned
                .retain(|resource, port| keeps_pin(&plan.model, resource, *port));
            return self.start_runtime(
                plan,
                Some(Launch {
                    carried: Some(carried),
                    deferred: BTreeSet::new(),
                }),
            );
        };
        let plan = driving.plan.clone();
        let deferred = driving.deferred.clone();
        // A pinned port stays only for a listener the plan keeps as it was
        // declared: a replaced listener gets its desired port.
        carried.pinned.retain(|resource, port| {
            driving.kinds.get(resource).map(String::as_str) != Some("replace")
                && keeps_pin(&plan.model, resource, *port)
        });
        self.runtime = None;
        self.start_runtime(
            plan,
            Some(Launch {
                carried: Some(carried),
                deferred,
            }),
        )
    }

    /// Start the run of `plan` with every machine, as a resumed revision.
    pub(super) fn start(&mut self, plan: Plan) -> Result<(), Failure> {
        self.start_runtime(plan, None)
    }

    fn start_runtime(&mut self, plan: Plan, launch: Option<Launch>) -> Result<(), Failure> {
        let id = plan.id.clone();
        let context = match self.contexts.remove(&id) {
            Some(context) => context,
            None => crate::apply::revision::Context::load(retained_plan(&self.root, &plan)?)?,
        };
        let expected = awaited(&context.model);
        let supervised = context
            .model
            .machines
            .iter()
            .filter(|machine| machine.variant() == "Supervise")
            .map(|machine| machine.id.clone())
            .collect();
        let adopted: BTreeSet<i64> = launch
            .as_ref()
            .and_then(|launch| launch.carried.as_ref())
            .map(|carried| {
                carried
                    .instances
                    .values()
                    .filter(|instance| instance.ready)
                    .map(|instance| i64::from(instance.pid))
                    .collect()
            })
            .unwrap_or_default();
        let (control, rx) = match launch.as_ref().and_then(|launch| launch.carried.as_ref()) {
            Some(carried) => (carried.tx.clone(), mpsc::channel().1),
            None => mpsc::channel(),
        };
        let tx = self.tx.clone();
        let operation_id = self
            .transition
            .as_ref()
            .map_or_else(|| id.clone(), |driving| driving.operation.clone());
        let operation_sink = Arc::new(Mutex::new(operation_id.clone()));
        let operation = Arc::clone(&operation_sink);
        let controller_plan = id.clone();
        let sink: crate::run::live::RecordSink = Box::new(move |record| {
            let (ack, wait) = mpsc::channel();
            let value = Json::parse(&record.line()).map_err(io::Error::other)?;
            let operation = operation
                .lock()
                .map_err(|_| io::Error::other("operation identity lock failed"))?
                .clone();
            tx.send(Message::Record(
                operation,
                controller_plan.clone(),
                value,
                ack,
            ))
            .map_err(|_| io::Error::other("host queue closed"))?;
            wait.recv().map_err(io::Error::other)?
        });
        let tx = self.tx.clone();
        let thread_control = control.clone();
        // A revision state of the same name can only be a crashed earlier
        // host's with this pid (a container's pid 1): its journal is never
        // reused. The new directory's entry, and that of `revisions/` when
        // it is new, are synced before anything runs in it (17.5).
        let revisions = self.root.join("revisions");
        std::fs::create_dir_all(&revisions)?;
        let runtime_state = loop {
            let candidate = revisions.join(format!(
                "{}-{}-{}",
                self.revision + 1,
                std::process::id(),
                crate::apply::NEXT_REQUEST.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&candidate) {
                Ok(()) => break candidate,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
        };
        for directory in [&revisions, &self.root] {
            std::fs::File::open(directory)?.sync_all()?;
        }
        save_json(
            &runtime_state.join("context.json"),
            &object([
                ("formatVersion", text(FORMAT)),
                ("planId", text(&id)),
                ("controllerAbi", text(crate::apply::plan::CONTROLLER_ABI)),
            ]),
        )?;
        let artifacts = Arc::new(Mutex::new(
            context
                .plan
                .artifacts
                .iter()
                .map(|(member, artifact)| (member.clone(), artifact.path.clone()))
                .collect::<BTreeMap<_, _>>(),
        ));
        let builder_artifacts = Arc::clone(&artifacts);
        let builder: crate::run::live::Builder = Arc::new(move || {
            builder_artifacts
                .lock()
                .map(|artifacts| artifacts.clone())
                .map_err(|_| "artifact manifest lock failed".to_string())
        });
        self.runtime = Some(Runtime {
            operation: operation_id,
            controller_plan: id.clone(),
            desired: id.clone(),
            model: crate::apply::plan::semantic_model(&context.plan.model),
            reducer: context.plan.reducer_revision.clone(),
            operation_sink,
            artifacts,
            minimum_generation: BTreeMap::new(),
            control,
            ready: BTreeSet::new(),
            expected,
            adopted,
            supervised,
            restore_to: None,
        });
        std::thread::spawn(move || {
            let initialized_tx = tx.clone();
            let initialized_id = id.clone();
            let started = Box::new(move || {
                initialized_tx
                    .send(Message::Initialized(initialized_id))
                    .map_err(|_| io::Error::other("host queue closed"))
            });
            let result = crate::apply::revision::run(
                context,
                &runtime_state,
                thread_control,
                rx,
                sink,
                started,
                builder,
                launch,
            );
            let _ = match result {
                Ok(crate::run::live::Ended::Handover(carried)) => {
                    tx.send(Message::Handover(id, Box::new(carried)))
                }
                Ok(crate::run::live::Ended::Stopped(code)) => {
                    tx.send(Message::Finished(id, Ok(code)))
                }
                Err(error) => tx.send(Message::Finished(id, Err(error.to_string()))),
            };
        });
        Ok(())
    }

    /// The controller of `id` initialized: a transition waiting for the
    /// desired revision's run is switched now; without a transition, a
    /// revision without awaited machines is complete now.
    pub(super) fn initialized(&mut self, id: &str) -> Result<(), Failure> {
        if !self
            .runtime
            .as_ref()
            .is_some_and(|runtime| runtime.controller_plan == id)
        {
            return Ok(());
        }
        if let Some(driving) = &mut self.transition {
            if driving.switching {
                driving.switching = false;
                driving.queue.push_back(transition_event("Switched", None));
            }
            return self.drive();
        }
        if self.active.as_deref() == Some(id)
            && self
                .runtime
                .as_ref()
                .is_some_and(|runtime| runtime.expected.is_empty())
        {
            self.complete(id)?;
        }
        Ok(())
    }

    /// The controller of `id` ended. Unless the host is shutting down, an
    /// active operation of another plan starts after a clean stop; any other
    /// end requires recovery.
    pub(super) fn finished(
        &mut self,
        id: &str,
        result: Result<i32, String>,
        shutdown: bool,
    ) -> Result<(), Failure> {
        if !self
            .runtime
            .as_ref()
            .is_some_and(|runtime| runtime.controller_plan == id)
        {
            return Ok(());
        }
        self.runtime = None;
        self.owned.clear();
        self.retiring.clear();
        if shutdown {
            return Ok(());
        }
        let reason = format!("revision controller ended before convergence: {result:?}");
        if self.effect_failed(&reason, "")? {
            if let Some(driving) = self.transition.take() {
                return self.reject_operation(&driving.operation, &reason);
            }
            return Ok(());
        }
        // An unresolved recovery stands (17.5): the end is kept beside its
        // reason, and nothing starts or fails another operation.
        if self.recovery_reason.is_some() {
            if let Some(standing) = self.standing() {
                let reason = if self.committed.as_deref() == Some(id) {
                    format!("committed revision controller ended: {result:?}")
                } else {
                    format!("revision controller ended before convergence: {result:?}")
                };
                self.keep_beside(&standing, id, &reason);
                self.persist()?;
            }
            return Ok(());
        }
        if let Some(active) = self.active.clone() {
            if active != id && matches!(result, Ok(0)) {
                let value = read_json(&self.root.join("plans").join(format!("{active}.json")))?;
                self.start(Plan::parse(&value).map_err(|reason| fail(3, reason))?)?;
            } else {
                self.reject_operation(
                    &active,
                    &format!("revision controller ended before convergence: {result:?}"),
                )?;
            }
        } else if let Some(committed) = self.committed.clone() {
            self.reject_operation(
                &committed,
                &format!("committed revision controller ended: {result:?}"),
            )?;
        }
        Ok(())
    }

    /// An effect of the transition in flight could not be carried out: step
    /// `EffectFailed`, whose `Abandon` fails the operation after it is
    /// journaled. False when no transition is in flight.
    fn effect_failed(&mut self, reason: &str, member: &str) -> Result<bool, Failure> {
        let Some(driving) = &mut self.transition else {
            return Ok(false);
        };
        driving.queue.push_back(effect_failed_event(reason, member));
        self.drive()?;
        Ok(true)
    }

    pub(super) fn record(
        &mut self,
        operation: &str,
        controller: &str,
        record: Json,
    ) -> Result<(), Failure> {
        self.append(
            "revisionRecord",
            object([
                ("operationId", text(operation)),
                ("controllerPlanId", text(controller)),
                ("record", record.clone()),
            ]),
        )?;
        let kind = record.get("kind").and_then(Json::as_str);
        if let Some(Json::Object(receipt)) = self.receipts.get_mut(operation)
            && matches!(
                receipt.get("state").and_then(Json::as_str),
                Some("accepted" | "applying")
            )
        {
            receipt.insert("progress".into(), record.clone());
        }
        if kind == Some("standing")
            && let Some(machine) = record.get("machine").and_then(Json::as_str)
        {
            self.standings.insert(machine.to_string(), record.clone());
        }
        // A machine that gave up blocks nothing: the operation it failed
        // waits for a corrective apply, and the other machines go on. A
        // failure kept beside it that a corrective does not resolve blocks.
        if kind == Some("effect") && self.recovery_reason.is_some() && !self.correctable() {
            return Err(fail(1, "effect dispatch blocked by unresolved recovery"));
        }
        if kind == Some("error") {
            self.reject_operation(
                operation,
                "revision reducer failed; inspect retained journal",
            )?;
        }
        let machine = record.get("machine").and_then(Json::as_str).unwrap_or("");
        let owns_machine = self
            .runtime
            .as_ref()
            .is_some_and(|runtime| runtime.expected.contains(machine));
        if matches!(kind, Some("signal" | "standing"))
            && owns_machine
            && let Some(runtime) = &mut self.runtime
        {
            runtime.ready.insert(machine.to_string());
        }
        if kind == Some("effect")
            && machine.ends_with(".supervisor")
            && record
                .get("value")
                .and_then(|v| v.get("variant"))
                .and_then(Json::as_str)
                == Some("Note")
            && texts(record.get("value").and_then(|v| v.get("fields")))
                .first()
                .map(String::as_str)
                == Some("gave_up")
            && let Some(driving) = &mut self.transition
        {
            driving
                .queue
                .push_back(transition_event("MemberGaveUp", Some(member_of(machine))));
        }
        let event = record
            .get("value")
            .and_then(|value| value.get("fields"))
            .and_then(|fields| fields.get("kind"));
        let variant = event
            .and_then(|value| value.get("variant"))
            .and_then(Json::as_str);
        let supervised = self
            .runtime
            .as_ref()
            .is_some_and(|runtime| runtime.supervised.contains(machine));
        if kind == Some("event") && supervised {
            self.lifecycle(operation, machine, event, variant)?;
        }
        if self.transition.is_some() {
            return self.drive();
        }
        if self.recovery_reason.is_none()
            && self.retiring.is_empty()
            && self.active.as_deref() == Some(operation)
            && self.runtime.as_ref().is_some_and(|runtime| {
                !runtime.expected.is_empty() && runtime.expected.is_subset(&runtime.ready)
            })
        {
            self.complete(operation)?;
        }
        Ok(())
    }

    /// Ownership and readiness from one lifecycle event of an awaited
    /// machine.
    fn lifecycle(
        &mut self,
        operation: &str,
        machine: &str,
        event: Option<&Json>,
        variant: Option<&str>,
    ) -> Result<(), Failure> {
        let member = member_of(machine);
        let fields = event
            .and_then(|value| value.get("fields"))
            .and_then(Json::as_array);
        let pid = fields
            .and_then(|fields| fields.first())
            .and_then(Json::as_i64);
        if let Some("Started") = variant {
            if let Some(runtime) = &mut self.runtime {
                runtime.ready.remove(machine);
            }
            if let Some(pid) = pid {
                if let Some(previous) = self.owned.get(member)
                    && let Some(old_pid) = previous.get("pid").and_then(Json::as_i64)
                    && old_pid != pid
                    && previous
                        .get("instance")
                        .and_then(Json::as_str)
                        .is_some_and(|instance| {
                            process_identity(old_pid).as_deref() == Some(instance)
                        })
                {
                    self.retiring.insert(old_pid.to_string(), previous.clone());
                    if let Some(driving) = &mut self.transition
                        && driving.start_first.contains(member)
                    {
                        driving.displaced.insert(member.to_string(), old_pid);
                    }
                }
                let generation = fields
                    .and_then(|fields| fields.get(1))
                    .and_then(Json::as_i64)
                    .unwrap_or(1);
                // An adopted instance reports `Started` again in the next
                // revision's run; it stays the instance of the operation
                // that started it.
                let origin = self
                    .owned
                    .get(member)
                    .filter(|previous| previous.get("pid").and_then(Json::as_i64) == Some(pid))
                    .and_then(|previous| previous.get("operationId"))
                    .cloned()
                    .unwrap_or_else(|| text(operation));
                self.owned.insert(
                    member.to_string(),
                    object([
                        ("pid", Json::Int(pid)),
                        ("ready", Json::Bool(false)),
                        ("operationId", origin),
                        ("instance", process_identity(pid).map_or(Json::Null, text)),
                        ("generation", Json::Int(generation)),
                    ]),
                );
            }
        }
        let matches_instance = self
            .owned
            .get(member)
            .and_then(|owned| owned.get("pid"))
            .and_then(Json::as_i64)
            == pid;
        let desired_generation = self
            .owned
            .get(member)
            .and_then(|owner| owner.get("generation"))
            .and_then(Json::as_i64)
            .is_some_and(|generation| {
                self.runtime
                    .as_ref()
                    .and_then(|runtime| runtime.minimum_generation.get(member))
                    .is_none_or(|minimum| generation >= *minimum)
            });
        if variant == Some("Ready") && matches_instance && desired_generation {
            let adopted = self
                .runtime
                .as_ref()
                .zip(pid)
                .is_some_and(|(runtime, pid)| runtime.adopted.contains(&pid));
            let desired = self
                .runtime
                .as_ref()
                .map(|runtime| runtime.desired.clone())
                .or_else(|| self.active.clone())
                .or_else(|| self.committed.clone());
            if !adopted && let Some(id) = desired {
                let manifest = read_json(&self.root.join("plans").join(format!("{id}.json")))?;
                let expected = manifest
                    .get("artifacts")
                    .and_then(|artifacts| artifacts.get(member))
                    .and_then(|artifact| artifact.get("digest"))
                    .and_then(Json::as_str);
                if let Some(pid) = pid {
                    let actual =
                        match crate::apply::digest::file(Path::new(&format!("/proc/{pid}/exe"))) {
                            Ok(digest) => digest,
                            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                                return Ok(());
                            }
                            Err(error) => {
                                return Err(fail(
                                    1,
                                    format!("cannot prove ready process artifact: {error}"),
                                ));
                            }
                        };
                    if expected != Some(actual.as_str()) {
                        let id = self.active.clone().unwrap_or(id);
                        let reason =
                            "ready process artifact differs from the admitted desired artifact";
                        if !self.effect_failed(reason, member)? {
                            self.reject_operation(&id, reason)?;
                        }
                        return Err(fail(1, "ready artifact identity mismatch"));
                    }
                }
            }
            if let Some(Json::Object(owned)) = self.owned.get_mut(member) {
                owned.insert("ready".into(), Json::Bool(true));
            }
            if let Some(runtime) = &mut self.runtime {
                runtime.ready.insert(machine.to_string());
            }
        }
        if matches!(variant, Some("Exited" | "Signalled")) && matches_instance {
            let was_ready = self
                .owned
                .remove(member)
                .and_then(|owner| owner.get("ready").and_then(Json::as_bool))
                == Some(true);
            if let Some(runtime) = &mut self.runtime {
                runtime.ready.remove(machine);
            }
            // A start-first replacement that ended before it was ready
            // never displaced the old instance, which still serves.
            let displaced = self
                .transition
                .as_ref()
                .and_then(|driving| driving.displaced.get(member).copied())
                .filter(|_| !was_ready);
            if let Some(old) = displaced
                && let Some(previous) = self.retiring.get(&old.to_string())
                && previous
                    .get("instance")
                    .and_then(Json::as_str)
                    .is_some_and(|instance| process_identity(old).as_deref() == Some(instance))
            {
                let previous = self.retiring.remove(&old.to_string()).unwrap();
                self.owned.insert(member.to_string(), previous);
            }
        }
        if matches!(variant, Some("Exited" | "Signalled"))
            && let Some(pid) = pid
        {
            self.retiring.remove(&pid.to_string());
        }
        if variant == Some("SpawnFailed")
            && (self.transition.is_some() || self.active.as_deref() == Some(operation))
        {
            if let Some(runtime) = &mut self.runtime {
                runtime.ready.remove(machine);
            }
            let reason = "a required owned process failed to start";
            if !self.effect_failed(reason, member)? {
                self.reject_operation(operation, reason)?;
            }
        }
        Ok(())
    }
}
