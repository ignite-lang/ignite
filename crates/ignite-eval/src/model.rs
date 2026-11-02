//! The system model `ignite plan` prints (`docs/slice.md` sections 15.4
//! and 15.5, D7): one system and at most one assembly, every property
//! evaluated, in the canonical JSON form that makes equal inputs equal
//! bytes. Owner: P3.
//!
//! Positions in the model and in `explain` are `file:line:column` with the
//! program file and its siblings as their file name (relative to the
//! program file's directory) and standard-library files as `std/<path>`
//! (O21). Nothing else about the machine enters the model (O22).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use ignite_syntax::ast::{Decl, Expr, ExprKind, Span, SystemDecl};
use ignite_syntax::diagnostic::Diagnostic;
use ignite_types::{
    AssemblyInfo, ExecutableInfo, MemberDeclaration, MemberInfo, PropertySource, QualName,
    QuantityKind, Reference, SystemInfo, TCP_LISTENER_DEFAULT_BACKLOG,
    TCP_LISTENER_DEFAULT_EXPOSURE, Ty, TypedProgram, field_default,
};

use crate::eval::Limit;
use crate::resolve::binding_fns;
use crate::{EvalError, Evaluator, Module, Modules, ResolutionEntry, Value, resolve};

/// The `formatVersion` of the model (O13).
pub const FORMAT_VERSION: &str = "ignite-model/0.1";

/// The disruption a `Roll` machine declares (`docs/slice.md` 16.12, O34):
/// the local backend has no socket handoff, so the plan says what a
/// rollout costs instead of simulating zero downtime (design 11.8).
pub const DISRUPTION: &str = "connections refused between listener close and rebind";

/// The module of the reducer interface (16.6).
pub const CONTROL_MODULE: &str = "std.control";

/// `std.control.companions(machine) -> List<Machine>`: the machines a
/// hook's result brings with it (a supervisor with a rollout policy brings
/// the member's `Roll` machine), since a policy module below `std.control`
/// cannot return a `Machine` itself (O45).
const COMPANIONS: &str = "companions";

/// The system model of one program (15.5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Model {
    /// The system's name.
    pub system_id: String,
    /// Every resource, component, contract and policy, sorted by `id`.
    pub members: Vec<ModelMember>,
    /// The selected assembly (15.6), `None` when the program declares none.
    pub assembly: Option<ModelAssembly>,
    /// Sorted by `from`, `kind`, `to`.
    pub edges: Vec<Edge>,
    /// Member id or property path (`Terminal.server.recovery`) to
    /// `file:line:column`, paths relative to the program file's directory,
    /// std files as `std/<path>` (O21). The system's and the assembly's
    /// ids map to their declarations too.
    pub source_map: BTreeMap<String, String>,
    /// The initial machines of the selected assembly (16.14), sorted by
    /// id; empty without an assembly.
    pub machines: Vec<ModelMachine>,
    /// The signals the selected assembly produces (16.14), sorted by ref;
    /// empty without an assembly.
    pub observations: Vec<ModelObservation>,
}

/// One member entry of the model (15.5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelMember {
    /// `<System>.<name>`.
    pub id: String,
    pub declaration: MemberDeclaration,
    /// Qualified name of the kind.
    pub kind: String,
    /// The executable's name when the kind has `K.main` (14.5).
    pub executable: Option<String>,
    /// Every property with its evaluated value, in the kind's field order
    /// (`explain` prints this order, the JSON sorts the keys, O24).
    pub properties: Vec<(String, Value)>,
    /// Names of the properties that took the kind's default, sorted.
    pub defaults: Vec<String>,
    /// Each unset `Signal<T>` field to its element type's name.
    pub outputs: Vec<(String, String)>,
}

/// The assembly entry of the model (15.6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelAssembly {
    /// The assembly's name.
    pub id: String,
    pub kind: String,
    /// The system the assembly deploys.
    pub system: String,
    pub properties: Vec<(String, Value)>,
    pub defaults: Vec<String>,
    /// One entry per resource and component, sorted by member id; `None`
    /// for an assembly kind that is not resolvable (15.6).
    pub resolution: Option<Vec<ResolutionEntry>>,
}

/// An edge of the model (15.5, O25). Ordered by `from`, the kind's name,
/// `to`: the order of the model's `edges` array.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Edge {
    pub from: String,
    pub kind: EdgeKind,
    pub to: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeKind {
    /// A bare member name in a property.
    Uses,
    /// A `Ref` value.
    References,
    /// A `Signal` value.
    Observes,
    /// From the resolution: a binding's key hosts an included member.
    Hosts,
    /// A resource to the hosted executable component that binds it
    /// (`Terminal.ingress` to `Terminal.server`; 16.14): the driver
    /// resolves a component's endpoint through it (16.9).
    Provisioning,
    /// A hosted member to the member whose readiness it waits for: a
    /// probe to its `Ref` target, a contract to its signal's producer
    /// (16.14); the routing of lifecycle events reads it (16.8, A4).
    Readiness,
    /// A member to its supervisor machine (16.14).
    Recovery,
}

impl EdgeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EdgeKind::Uses => "uses",
            EdgeKind::References => "references",
            EdgeKind::Observes => "observes",
            EdgeKind::Hosts => "hosts",
            EdgeKind::Provisioning => "provisioning",
            EdgeKind::Readiness => "readiness",
            EdgeKind::Recovery => "recovery",
        }
    }

    /// The passive form `explain` prints for an incoming edge.
    pub fn passive(self) -> &'static str {
        match self {
            EdgeKind::Uses => "used by",
            EdgeKind::References => "referenced by",
            EdgeKind::Observes => "observed by",
            EdgeKind::Hosts => "hosted by",
            EdgeKind::Provisioning => "provisions",
            EdgeKind::Readiness => "awaited by",
            EdgeKind::Recovery => "recovers",
        }
    }
}

impl PartialOrd for EdgeKind {
    fn partial_cmp(&self, other: &EdgeKind) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for EdgeKind {
    /// By name, as the JSON sorts.
    fn cmp(&self, other: &EdgeKind) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

/// One entry of the model's `machines` array (`docs/slice.md` 16.14): the
/// initial `std.control.Machine` value a `machine` hook returned for a
/// hosted member or contract, the member it belongs to, the signal refs it
/// subscribes to and, for a `Roll` machine, its declared disruption
/// (16.12, O34).
///
/// The hook is `K.machine(self, id: Text) -> std.control.Machine`, called
/// with the member's id (`Terminal.server`) on the member's kind and on
/// every record-typed property value whose type's module declares one
/// (`std.policy.DrainRollout.machine` gives `Terminal.server.rollout`); the
/// hook forms the machine id `<member>.<role>` in its payload's `id` field,
/// which is where the model reads it (16.6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelMachine {
    /// `<member path>.<role>`, e.g. `Terminal.server.supervisor`.
    pub id: String,
    /// The initial `Machine` value.
    pub machine: Value,
    /// The member or contract id the machine belongs to.
    pub member: String,
    /// Signal refs, sorted: the `Signal` values among the member's
    /// properties.
    pub subscribes: Vec<String>,
    /// [`DISRUPTION`] for a `Roll` machine.
    pub disruption: Option<String>,
}

impl ModelMachine {
    /// The `Machine` variant: `Supervise`, `Schedule`, `Evaluate` or
    /// `Roll`.
    pub fn variant(&self) -> &str {
        match &self.machine {
            Value::Enum { variant, .. } => variant,
            _ => "",
        }
    }

    /// The machine's state record, the variant's payload.
    pub fn payload(&self) -> Option<&Value> {
        match &self.machine {
            Value::Enum { fields, .. } => fields.first(),
            _ => None,
        }
    }

    /// The `name` field of the payload, when it is a `Text`.
    pub fn text_field(&self, name: &str) -> Option<&str> {
        match self.payload()? {
            Value::Record { fields, .. } => fields.iter().find_map(|(f, v)| match v {
                Value::Text(text) if f == name => Some(text.as_str()),
                _ => None,
            }),
            _ => None,
        }
    }
}

/// One entry of the model's `observations` array (16.14): a signal the
/// assembly produces, its producer member, its element type and the
/// producer's `interval` property (`Unit` when the producer has none).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelObservation {
    /// The signal's ref text, e.g. `Terminal.promptProbe.attempts`.
    pub reference: String,
    pub producer: String,
    /// The qualified element type, e.g. `std.sli.ProbeAttempt`.
    pub element: String,
    pub interval: Value,
}

/// The record or enum name of a kind; `None` for a builtin kind such as
/// `TcpListener`, which declares no hooks.
fn kind_name(kind: &Ty) -> Option<&QualName> {
    match kind {
        Ty::Record(q) | Ty::GenericRecord(q, _) | Ty::Enum(q) | Ty::GenericEnum(q, _) => Some(q),
        _ => None,
    }
}

/// The hook `<Owner>.<hook>` with `arity` parameters in `owner`'s module,
/// when the module declares one (16.6: the compiler knows the hook names
/// `machine` and `validate`, O18).
fn hook_fn(modules: &Modules<'_>, owner: &QualName, hook: &str, arity: usize) -> Option<QualName> {
    if owner.is_builtin() {
        return None;
    }
    let module = modules.get(&owner.module)?;
    let name = format!("{}.{hook}", owner.name);
    let declared = module.program.decls.iter().any(|decl| {
        matches!(decl, Decl::Fn(f) if f.qualified_name() == name && f.params.len() == arity && !f.test_only)
    });
    declared.then(|| QualName::new(owner.module.clone(), name))
}

/// The member's own record value as a hook receives it: every field of
/// the kind in declaration order, an unset `Signal<T>` output as its
/// reference (6.1).
fn member_value(
    program: &TypedProgram,
    system: &SystemInfo,
    info: &MemberInfo,
    member: &ModelMember,
) -> Option<Value> {
    let name = kind_name(&info.kind)?;
    let declared = program.record_fields(name)?;
    let fields = declared
        .iter()
        .filter_map(|(field, _)| {
            if let Some((_, value)) = member.properties.iter().find(|(f, _)| f == field) {
                return Some((field.clone(), value.clone()));
            }
            member.outputs.iter().find(|(f, _)| f == field).map(|_| {
                let reference = Reference::Signal {
                    system: system.name.clone(),
                    member: info.name.clone(),
                    field: field.clone(),
                };
                (field.clone(), Value::Reference(reference))
            })
        })
        .collect();
    Some(Value::Record {
        name: name.clone(),
        fields,
    })
}

/// The `Signal` references among `value`, as texts.
fn signal_refs(value: &Value, out: &mut BTreeSet<String>) {
    match value {
        Value::Reference(reference @ Reference::Signal { .. }) => {
            out.insert(reference.text());
        }
        Value::List(items) | Value::Tuple(items) | Value::Enum { fields: items, .. } => {
            items.iter().for_each(|item| signal_refs(item, out));
        }
        Value::Record { fields, .. } => fields.iter().for_each(|(_, v)| signal_refs(v, out)),
        Value::Map(entries) => entries.iter().for_each(|(k, v)| {
            signal_refs(k, out);
            signal_refs(v, out);
        }),
        _ => {}
    }
}

/// Set field `field` of the payload record of `machine` to `value`.
fn set_payload_field(machine: &mut Value, field: &str, value: Value) {
    if let Value::Enum { fields, .. } = machine
        && let Some(Value::Record { fields, .. }) = fields.first_mut()
        && let Some((_, slot)) = fields.iter_mut().find(|(f, _)| f == field)
    {
        *slot = value;
    }
}

/// The machines of `system` under the resolution `resolution` (16.14): the
/// `machine` hooks evaluated for every hosted member and contract, sorted
/// by id (see [`ModelMachine`]). A machine whose payload has `signal` and
/// `interval` fields gets `interval` from the observation of that signal,
/// because a contract's kind does not know its producer's interval.
/// Diagnostics are E115, E117 or E118 at the member's declaration.
pub fn machines(
    evaluator: &mut Evaluator<'_>,
    program: &TypedProgram,
    system: &SystemInfo,
    members: &[ModelMember],
    resolution: &[ResolutionEntry],
    observations: &[ModelObservation],
) -> Result<Vec<ModelMachine>, Vec<Diagnostic>> {
    let mut machines = Vec::new();
    let mut diagnostics = Vec::new();
    for info in &system.members {
        let Some(member) = members.iter().find(|m| m.id == info.id) else {
            continue;
        };
        let hosted = info.declaration == MemberDeclaration::Contract
            || resolution.iter().any(|entry| entry.member == info.id);
        if !hosted {
            continue;
        }
        let modules = evaluator.modules();
        let mut hooks: Vec<(QualName, Value)> = Vec::new();
        if let Some(hook) = kind_name(&info.kind).and_then(|q| hook_fn(modules, q, "machine", 2))
            && let Some(this) = member_value(program, system, info, member)
        {
            hooks.push((hook, this));
        }
        for (_, value) in &member.properties {
            if let Value::Record { name, .. } = value
                && let Some(hook) = hook_fn(modules, name, "machine", 2)
            {
                hooks.push((hook, value.clone()));
            }
        }
        if hooks.is_empty() {
            continue;
        }
        let mut subscribes = BTreeSet::new();
        for (_, value) in &member.properties {
            signal_refs(value, &mut subscribes);
        }
        let span = Span::new(info.offset, info.offset);
        let companions = modules
            .get(CONTROL_MODULE)
            .filter(|module| {
                module.program.decls.iter().any(|decl| {
                    matches!(decl, Decl::Fn(f) if f.qualified_name() == COMPANIONS && f.params.len() == 1)
                })
            })
            .map(|_| QualName::new(CONTROL_MODULE, COMPANIONS));
        let mut results: Vec<(String, Value)> = Vec::new();
        for (hook, this) in hooks {
            let path = format!("{}.{}", info.id, hook.name);
            let machine = match evaluator.call(&hook, vec![this, Value::Text(info.id.clone())]) {
                Ok(machine) => machine,
                Err(error) => {
                    diagnostics.push(eval_diagnostic(&error, &path, span));
                    continue;
                }
            };
            if let Some(companions) = &companions {
                match evaluator.call(companions, vec![machine.clone()]) {
                    Ok(Value::List(items)) => {
                        results.extend(items.into_iter().map(|m| (format!("{path} companion"), m)));
                    }
                    Ok(_) => {}
                    Err(error) => diagnostics.push(eval_diagnostic(&error, &path, span)),
                }
            }
            results.push((path, machine));
        }
        for (path, mut machine) in results {
            let entry = ModelMachine {
                id: String::new(),
                machine: Value::Unit,
                member: info.id.clone(),
                subscribes: subscribes.iter().cloned().collect(),
                disruption: None,
            };
            let probe = ModelMachine {
                machine: machine.clone(),
                ..entry.clone()
            };
            let Some(id) = probe.text_field("id").map(str::to_string) else {
                diagnostics.push(Diagnostic::new(
                    "E117",
                    span,
                    format!(
                        "evaluating {path}: the machine `{}` has no `id` text in its state",
                        probe.variant()
                    ),
                ));
                continue;
            };
            if let Some(signal) = probe.text_field("signal").map(str::to_string)
                && let Some(observation) = observations.iter().find(|o| o.reference == signal)
            {
                set_payload_field(&mut machine, "interval", observation.interval.clone());
            }
            let disruption = (probe.variant() == "Roll").then(|| DISRUPTION.to_string());
            machines.push(ModelMachine {
                id,
                machine,
                disruption,
                ..entry
            });
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    machines.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(machines)
}

/// The observations of `system` under `resolution` (16.14): one per
/// `Signal<T>` output of a hosted member, sorted by ref, with the
/// producer's `interval` property (`Unit` when it has none).
pub fn observations(
    _program: &TypedProgram,
    _system: &SystemInfo,
    members: &[ModelMember],
    resolution: &[ResolutionEntry],
) -> Result<Vec<ModelObservation>, Vec<Diagnostic>> {
    let mut observations: Vec<ModelObservation> = resolution
        .iter()
        .filter_map(|entry| members.iter().find(|m| m.id == entry.member))
        .flat_map(|member| {
            let interval = member
                .properties
                .iter()
                .find(|(f, _)| f == "interval")
                .map_or(Value::Unit, |(_, v)| v.clone());
            member
                .outputs
                .iter()
                .map(move |(field, element)| ModelObservation {
                    reference: format!("{}.{field}", member.id),
                    producer: member.id.clone(),
                    element: element.clone(),
                    interval: interval.clone(),
                })
        })
        .collect();
    observations.sort_by(|a, b| a.reference.cmp(&b.reference));
    Ok(observations)
}

/// The edge classes of 16.14 for `system` under `resolution`:
/// `provisioning` from each resource a hosted executable binds (its
/// [`ExecutableInfo::resources`]) to that executable, `readiness` from a
/// hosted member to each member its `Ref` values name and from a contract
/// to the producer of each `Signal` it names, `recovery` from a member to
/// its `Supervise` machine.
fn control_edges(
    program: &TypedProgram,
    system: &SystemInfo,
    members: &[ModelMember],
    resolution: &[ResolutionEntry],
    machines: &[ModelMachine],
    edges: &mut BTreeSet<Edge>,
) {
    let mut edge = |kind: EdgeKind, from: String, to: String| {
        edges.insert(Edge { from, kind, to });
    };
    for entry in resolution {
        let component = entry
            .member
            .strip_prefix(&format!("{}.", system.name))
            .unwrap_or(&entry.member);
        if let Some(executable) = program
            .executables()
            .iter()
            .find(|e| e.system == system.name && e.component == component)
        {
            for (resource, _) in &executable.resources {
                edge(
                    EdgeKind::Provisioning,
                    format!("{}.{resource}", system.name),
                    entry.member.clone(),
                );
            }
        }
    }
    for member in members {
        let hosted = member.declaration == MemberDeclaration::Contract
            || resolution.iter().any(|entry| entry.member == member.id);
        if !hosted {
            continue;
        }
        let mut targets = BTreeSet::new();
        for (_, value) in &member.properties {
            readiness_targets(value, &mut targets);
        }
        for target in targets {
            edge(EdgeKind::Readiness, member.id.clone(), target);
        }
    }
    for machine in machines {
        if machine.variant() == "Supervise" {
            edge(
                EdgeKind::Recovery,
                machine.member.clone(),
                machine.id.clone(),
            );
        }
    }
}

/// The members whose readiness `value` depends on: the member of each
/// `Ref`, the producer of each `Signal`.
fn readiness_targets(value: &Value, out: &mut BTreeSet<String>) {
    match value {
        Value::Reference(Reference::Member { system, member })
        | Value::Reference(Reference::Signal { system, member, .. }) => {
            out.insert(format!("{system}.{member}"));
        }
        Value::List(items) | Value::Tuple(items) | Value::Enum { fields: items, .. } => {
            items.iter().for_each(|item| readiness_targets(item, out));
        }
        Value::Record { fields, .. } => fields.iter().for_each(|(_, v)| readiness_targets(v, out)),
        Value::Map(entries) => entries.iter().for_each(|(k, v)| {
            readiness_targets(k, out);
            readiness_targets(v, out);
        }),
        _ => {}
    }
}

/// The qualified name of a type as the model prints it (15.4): records
/// and enums as `module.Name`, builtin types by their bare name, type
/// arguments qualified the same way.
pub fn type_name(ty: &Ty) -> String {
    let list = |args: &[Ty]| args.iter().map(type_name).collect::<Vec<_>>().join(", ");
    match ty {
        Ty::Record(q) | Ty::Enum(q) => q.to_string(),
        Ty::GenericRecord(q, args) | Ty::GenericEnum(q, args) => format!("{q}<{}>", list(args)),
        Ty::List(t) if **t == Ty::Byte => "Bytes".to_string(),
        Ty::List(t) => format!("List<{}>", type_name(t)),
        Ty::Option(t) => format!("Option<{}>", type_name(t)),
        Ty::Result(ok, err) => format!("Result<{}, {}>", type_name(ok), type_name(err)),
        Ty::Tuple(items) => format!("({})", list(items)),
        Ty::Fn(params, ret) => format!("fn({}) -> {}", list(params), type_name(ret)),
        Ty::Store(t) => format!("Store<{}>", type_name(t)),
        Ty::Map(k, v) => format!("Map<{}, {}>", type_name(k), type_name(v)),
        Ty::Ref(Some(k)) => format!("Ref<{}>", type_name(k)),
        Ty::Signal(t) => format!("Signal<{}>", type_name(t)),
        other => other.to_string(),
    }
}

/// `file:line:column` of `offset` in `module` (O21).
pub fn position(module: &Module<'_>, offset: usize) -> String {
    let (line, col) = module.source.line_col(offset);
    format!("{}:{line}:{col}", file_label(module))
}

/// The file of `module` as the model names it: `std/<path>.ignite` for a
/// standard-library module, else the file's name.
fn file_label(module: &Module<'_>) -> String {
    match module.path.strip_prefix("std.") {
        Some(rest) => format!("std/{}.ignite", rest.replace('.', "/")),
        None => Path::new(&module.source.path)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| module.source.path.clone()),
    }
}

/// The diagnostic of an evaluation that produced no value, at `span`:
/// E115, E117 or E118 (15.3) for the property `path`.
pub fn eval_diagnostic(error: &EvalError, path: &str, span: Span) -> Diagnostic {
    match error {
        EvalError::Limit { limit, running } => {
            let limit = match limit {
                Limit::Fuel(n) => format!("the fuel limit of {n} units"),
                Limit::Nodes(n) => format!("the value-size limit of {n} nodes"),
                Limit::Depth(n) => format!("the call-depth limit of {n} calls"),
            };
            Diagnostic::new(
                "E115",
                span,
                format!("evaluating {path}: {limit} exceeded in {running}"),
            )
        }
        EvalError::Failed { report } => {
            Diagnostic::new("E117", span, format!("evaluating {path}: {report}"))
        }
        EvalError::Unencodable { what } => Diagnostic::new(
            "E118",
            span,
            format!("evaluating {path}: {what} cannot be encoded in the model"),
        ),
    }
}

/// The first part of `value` the model cannot encode (15.4): a closure or
/// an `IoError`.
fn unencodable(value: &Value) -> Option<String> {
    match value {
        Value::Closure { module, offset, .. } => {
            Some(format!("a closure ({module} at byte {offset})"))
        }
        Value::IoError { message, .. } => Some(format!("an IoError ({message:?})")),
        Value::List(items) | Value::Tuple(items) | Value::Enum { fields: items, .. } => {
            items.iter().find_map(unencodable)
        }
        Value::Record { fields, .. } => fields.iter().find_map(|(_, v)| unencodable(v)),
        Value::Map(entries) => entries
            .iter()
            .find_map(|(k, v)| unencodable(k).or_else(|| unencodable(v))),
        _ => None,
    }
}

/// The `references` and `observes` edges from `from` to the references in
/// `value`, and `uses` for the members it embeds (15.4, 15.5).
fn value_edges(from: &str, value: &Value, edges: &mut BTreeSet<Edge>) {
    let mut edge = |kind: EdgeKind, to: String| {
        edges.insert(Edge {
            from: from.to_string(),
            kind,
            to,
        });
    };
    match value {
        Value::Member { system, member } => edge(EdgeKind::Uses, format!("{system}.{member}")),
        Value::Reference(reference @ Reference::Member { .. }) => {
            edge(EdgeKind::References, reference.text())
        }
        Value::Reference(reference @ Reference::Signal { .. }) => {
            edge(EdgeKind::Observes, reference.text())
        }
        Value::List(items) | Value::Tuple(items) | Value::Enum { fields: items, .. } => {
            for item in items {
                value_edges(from, item, edges);
            }
        }
        Value::Record { fields, .. } => {
            for (_, item) in fields {
                value_edges(from, item, edges);
            }
        }
        Value::Map(entries) => {
            for (k, v) in entries {
                value_edges(from, k, edges);
                value_edges(from, v, edges);
            }
        }
        _ => {}
    }
}

/// What the evaluation of one system or assembly produced so far.
struct Builder<'a, 'p> {
    evaluator: &'a mut Evaluator<'p>,
    /// The program's module.
    module: Module<'p>,
    source_map: BTreeMap<String, String>,
    edges: BTreeSet<Edge>,
    diagnostics: Vec<Diagnostic>,
    /// The evaluated members of each system, by system name, for the
    /// assembly's validation (16.17).
    members: BTreeMap<String, Vec<ModelMember>>,
    /// Of the last assembly that resolved (16.14).
    machines: Vec<ModelMachine>,
    observations: Vec<ModelObservation>,
}

impl<'a, 'p> Builder<'a, 'p> {
    fn new(evaluator: &'a mut Evaluator<'p>, program: &TypedProgram) -> Builder<'a, 'p> {
        let module = *evaluator
            .modules()
            .get(&program.module)
            .expect("the program's module is one of the evaluator's modules");
        Builder {
            evaluator,
            module,
            source_map: BTreeMap::new(),
            edges: BTreeSet::new(),
            diagnostics: Vec::new(),
            members: BTreeMap::new(),
            machines: Vec::new(),
            observations: Vec::new(),
        }
    }

    fn position(&self, offset: usize) -> String {
        position(&self.module, offset)
    }

    /// The value of property `field` of the declaration `owner` (a member
    /// id or an assembly name) whose declaration starts at `offset`:
    /// evaluated from the declaration (`set` gives the expression), from
    /// the kind's default, or the component's name (15.9). Records the
    /// property's position; `None` after a diagnostic.
    fn property(
        &mut self,
        owner: &str,
        name: &str,
        field: &str,
        source: &PropertySource,
        set: Option<&'p Expr>,
        offset: usize,
    ) -> Option<(Value, bool)> {
        let path = format!("{owner}.{field}");
        let owner_span = Span::new(offset, offset);
        let (value, defaulted) = match source {
            PropertySource::Set(_) => {
                let expr = set?;
                self.source_map
                    .insert(path.clone(), self.position(expr.span.start));
                let types = self.module.types;
                match self.evaluator.evaluate(types, expr) {
                    Ok(value) => (value, false),
                    Err(error) => {
                        self.diagnostics
                            .push(eval_diagnostic(&error, &path, expr.span));
                        return None;
                    }
                }
            }
            PropertySource::Default(q) if q.is_builtin() => {
                self.source_map.insert(path.clone(), self.position(offset));
                let value = match q.name.as_str() {
                    "TcpListener.exposure" => Value::Enum {
                        name: QualName::builtin("Exposure"),
                        variant: TCP_LISTENER_DEFAULT_EXPOSURE.to_string(),
                        fields: Vec::new(),
                    },
                    "TcpListener.backlog" => Value::Int(TCP_LISTENER_DEFAULT_BACKLOG),
                    _ => return None,
                };
                (value, true)
            }
            PropertySource::Default(q) => {
                let module = *self.evaluator.modules().get(&q.module)?;
                let (record, field) = q.name.split_once('.')?;
                let expr = field_default(module.program, record, field)?;
                self.source_map
                    .insert(path.clone(), position(&module, expr.span.start));
                match self.evaluator.evaluate(module.types, expr) {
                    Ok(value) => (value, true),
                    Err(error) => {
                        self.diagnostics
                            .push(eval_diagnostic(&error, &path, owner_span));
                        return None;
                    }
                }
            }
            PropertySource::ComponentName => {
                self.source_map.insert(path.clone(), self.position(offset));
                (Value::Text(name.to_string()), true)
            }
        };
        if let Some(what) = unencodable(&value) {
            let span = set.map_or(owner_span, |expr| expr.span);
            self.diagnostics.push(eval_diagnostic(
                &EvalError::Unencodable { what },
                &path,
                span,
            ));
            return None;
        }
        Some((value, defaulted))
    }

    /// `uses` edges from `from` for every bare member name of `system` in
    /// `expr` that is not a reference (a reference is an edge of its value).
    fn uses(&mut self, from: &str, system: &SystemInfo, expr: &Expr) {
        let types = self.module.types;
        match &expr.kind {
            ExprKind::Name(name) => {
                if types.reference(expr).is_none() && system.member(&name.name).is_some() {
                    self.edges.insert(Edge {
                        from: from.to_string(),
                        kind: EdgeKind::Uses,
                        to: format!("{}.{}", system.name, name.name),
                    });
                }
            }
            ExprKind::Record { fields, .. } => {
                for field in fields {
                    self.uses(from, system, &field.value);
                }
            }
            ExprKind::RecordUpdate { fields, base, .. } => {
                for field in fields {
                    self.uses(from, system, &field.value);
                }
                self.uses(from, system, base);
            }
            ExprKind::Brace { entries } => {
                for entry in entries {
                    self.uses(from, system, &entry.value);
                }
            }
            ExprKind::List(items) | ExprKind::Tuple(items) => {
                for item in items {
                    self.uses(from, system, item);
                }
            }
            ExprKind::Call { args, .. } => {
                for arg in args {
                    self.uses(from, system, arg);
                }
            }
            ExprKind::Method { receiver, args, .. } => {
                self.uses(from, system, receiver);
                for arg in args {
                    self.uses(from, system, arg);
                }
            }
            ExprKind::NamedArg { value, .. } => self.uses(from, system, value),
            ExprKind::Field { receiver, .. } if types.reference(expr).is_none() => {
                self.uses(from, system, receiver);
            }
            ExprKind::Unary { operand, .. } => self.uses(from, system, operand),
            ExprKind::Binary { left, right, .. } => {
                self.uses(from, system, left);
                self.uses(from, system, right);
            }
            _ => {}
        }
    }

    /// The entry of `member` of `system`, evaluated; `None` after a
    /// diagnostic.
    fn member(
        &mut self,
        system: &SystemInfo,
        decl: &'p SystemDecl,
        member: &MemberInfo,
        executables: &[ExecutableInfo],
    ) -> Option<ModelMember> {
        self.source_map
            .insert(member.id.clone(), self.position(member.offset));
        let mut properties = Vec::new();
        let mut defaults = Vec::new();
        let mut complete = true;
        for (field, source) in &member.properties {
            let set = member.property(decl, field);
            if let Some(expr) = set {
                self.uses(&member.id, system, expr);
            }
            match self.property(&member.id, &member.name, field, source, set, member.offset) {
                Some((value, defaulted)) => {
                    value_edges(&member.id, &value, &mut self.edges);
                    if defaulted {
                        defaults.push(field.clone());
                    }
                    properties.push((field.clone(), value));
                }
                None => complete = false,
            }
        }
        if !complete {
            return None;
        }
        defaults.sort();
        let executable = executables
            .iter()
            .find(|e| e.system == system.name && e.component == member.name)
            .map(|e| e.executable.clone());
        Some(ModelMember {
            id: member.id.clone(),
            declaration: member.declaration,
            kind: type_name(&member.kind),
            executable,
            properties,
            defaults,
            outputs: member
                .outputs
                .iter()
                .map(|(field, ty)| (field.clone(), type_name(ty)))
                .collect(),
        })
    }

    /// Every member of `system`, evaluated and sorted by id.
    fn system(&mut self, program: &TypedProgram, system: &SystemInfo) -> Vec<ModelMember> {
        let Some(decl) = system.decl(self.module.program) else {
            return Vec::new();
        };
        self.source_map
            .insert(system.name.clone(), self.position(system.offset));
        let mut members: Vec<ModelMember> = system
            .members
            .iter()
            .filter_map(|member| self.member(system, decl, member, program.executables()))
            .collect();
        members.sort_by(|a, b| a.id.cmp(&b.id));
        if let Err(diagnostics) =
            crate::validate::validate_system(self.evaluator, program, system, &members)
        {
            self.diagnostics.extend(diagnostics);
        }
        self.members.insert(system.name.clone(), members.clone());
        members
    }

    /// The assembly entry of `assembly`, evaluated, with its resolution
    /// when `system` is given and the kind is resolvable (15.6); `None`
    /// after a diagnostic.
    fn assembly(
        &mut self,
        program: &TypedProgram,
        assembly: &AssemblyInfo,
        system: Option<&SystemInfo>,
    ) -> Option<ModelAssembly> {
        self.source_map
            .insert(assembly.name.clone(), self.position(assembly.offset));
        let mut properties = Vec::new();
        let mut defaults = Vec::new();
        let mut complete = true;
        for (field, source) in &assembly.properties {
            let set = assembly.property(self.module.program, field);
            match self.property(
                &assembly.name,
                &assembly.name,
                field,
                source,
                set,
                assembly.offset,
            ) {
                Some((value, defaulted)) => {
                    if defaulted {
                        defaults.push(field.clone());
                    }
                    properties.push((field.clone(), value));
                }
                None => complete = false,
            }
        }
        if !complete {
            return None;
        }
        defaults.sort();
        let resolvable = binding_fns(program, assembly, self.evaluator.modules()).is_some();
        let bindings = properties
            .iter()
            .find(|(field, _)| field == "bindings")
            .map(|(_, value)| value.clone());
        let resolution = match (system, bindings) {
            (Some(system), Some(bindings)) if resolvable => {
                match resolve(self.evaluator, program, system, assembly, &bindings) {
                    Ok(entries) => {
                        let members = self.members.get(&system.name).cloned().unwrap_or_default();
                        if let Err(diagnostics) = crate::validate::validate(
                            self.evaluator,
                            program,
                            system,
                            &members,
                            assembly,
                            &entries,
                        ) {
                            self.diagnostics.extend(diagnostics);
                            return None;
                        }
                        if !self.control(program, system, &members, &entries) {
                            return None;
                        }
                        for entry in &entries {
                            if entry.via != entry.member {
                                self.edges.insert(Edge {
                                    from: entry.via.clone(),
                                    kind: EdgeKind::Hosts,
                                    to: entry.member.clone(),
                                });
                            }
                        }
                        Some(entries)
                    }
                    Err(diagnostics) => {
                        self.diagnostics.extend(diagnostics);
                        return None;
                    }
                }
            }
            _ => None,
        };
        Some(ModelAssembly {
            id: assembly.name.clone(),
            kind: type_name(&assembly.kind),
            system: system.map(|s| s.name.clone()).unwrap_or_default(),
            properties,
            defaults,
            resolution,
        })
    }

    /// The observations, machines and edge classes of 16.14 for `system`
    /// under the resolution `entries`; `false` after a diagnostic.
    fn control(
        &mut self,
        program: &TypedProgram,
        system: &SystemInfo,
        members: &[ModelMember],
        entries: &[ResolutionEntry],
    ) -> bool {
        let observations = match observations(program, system, members, entries) {
            Ok(observations) => observations,
            Err(diagnostics) => {
                self.diagnostics.extend(diagnostics);
                return false;
            }
        };
        let machines = match machines(
            self.evaluator,
            program,
            system,
            members,
            entries,
            &observations,
        ) {
            Ok(machines) => machines,
            Err(diagnostics) => {
                self.diagnostics.extend(diagnostics);
                return false;
            }
        };
        control_edges(
            program,
            system,
            members,
            entries,
            &machines,
            &mut self.edges,
        );
        self.machines = machines;
        self.observations = observations;
        true
    }
}

/// The system and assembly `ignite plan` selects (15.6): the assembly
/// named by `name`, else the program's only one, and the system it
/// deploys; without assemblies the program's only system. E114 otherwise.
fn select<'t>(
    program: &'t TypedProgram,
    name: Option<&str>,
) -> Result<(&'t SystemInfo, Option<&'t AssemblyInfo>), Vec<Diagnostic>> {
    let systems = program.systems();
    let assemblies = program.assemblies();
    let names = |items: &[String]| items.join(", ");
    let assembly = match name {
        Some(name) => match assemblies.iter().find(|a| a.name == name) {
            Some(assembly) => Some(assembly),
            None => {
                let known: Vec<String> = assemblies.iter().map(|a| a.name.clone()).collect();
                let detail = if known.is_empty() {
                    "the program declares no assembly".to_string()
                } else {
                    format!("the program declares {}", names(&known))
                };
                return Err(vec![Diagnostic::new(
                    "E114",
                    Span::new(0, 0),
                    format!("no assembly named `{name}`: {detail}"),
                )]);
            }
        },
        None => match assemblies {
            [] => None,
            [one] => Some(one),
            [_, second, ..] => {
                let known: Vec<String> = assemblies.iter().map(|a| a.name.clone()).collect();
                return Err(vec![Diagnostic::new(
                    "E114",
                    Span::new(second.offset, second.offset),
                    format!(
                        "several assemblies ({}) and no `--assembly`: select one",
                        names(&known)
                    ),
                )]);
            }
        },
    };
    let system = match assembly {
        Some(assembly) => {
            let Some(system_name) = assembly.system.as_deref() else {
                return Err(vec![Diagnostic::new(
                    "E114",
                    Span::new(assembly.offset, assembly.offset),
                    format!(
                        "assembly `{}` names no system in its `system` property",
                        assembly.name
                    ),
                )]);
            };
            match systems.iter().find(|s| s.name == system_name) {
                Some(system) => system,
                None => {
                    return Err(vec![Diagnostic::new(
                        "E114",
                        Span::new(assembly.offset, assembly.offset),
                        format!(
                            "assembly `{}` deploys `{system_name}`, which is not a system of this program",
                            assembly.name
                        ),
                    )]);
                }
            }
        }
        None => match systems {
            [one] => one,
            [] => {
                return Err(vec![Diagnostic::new(
                    "E114",
                    Span::new(0, 0),
                    "the program declares no system to plan",
                )]);
            }
            [_, second, ..] => {
                let known: Vec<String> = systems.iter().map(|s| s.name.clone()).collect();
                return Err(vec![Diagnostic::new(
                    "E114",
                    Span::new(second.offset, second.offset),
                    format!(
                        "several systems ({}) and no assembly: declare an assembly and select it",
                        names(&known)
                    ),
                )]);
            }
        },
    };
    Ok((system, assembly))
}

impl Model {
    /// The evaluation and resolution `ignite check` runs (O20): every
    /// property of every system and assembly of `program` is evaluated,
    /// and every assembly whose `system` names a system of the program is
    /// resolved; E110 to E113, E115, E117 and E118 as `model` reports them,
    /// never E114.
    pub fn check(
        evaluator: &mut Evaluator<'_>,
        program: &TypedProgram,
    ) -> Result<(), Vec<Diagnostic>> {
        if program.systems().is_empty() && program.assemblies().is_empty() {
            return Ok(());
        }
        evaluator.with_stack(|evaluator| {
            let mut builder = Builder::new(evaluator, program);
            for system in program.systems() {
                builder.system(program, system);
            }
            for assembly in program.assemblies() {
                let system = assembly
                    .system
                    .as_deref()
                    .and_then(|name| program.systems().iter().find(|s| s.name == name));
                builder.assembly(program, assembly, system);
            }
            if builder.diagnostics.is_empty() {
                Ok(())
            } else {
                Err(builder.diagnostics)
            }
        })
    }

    /// The members with declaration `declaration`, in id order.
    pub fn members_of(&self, declaration: MemberDeclaration) -> impl Iterator<Item = &ModelMember> {
        self.members
            .iter()
            .filter(move |m| m.declaration == declaration)
    }

    pub fn member(&self, id: &str) -> Option<&ModelMember> {
        self.members.iter().find(|m| m.id == id)
    }

    /// The canonical JSON of 15.5: UTF-8, keys in byte order at every
    /// level, two-space indentation, one key or item per line, `[]` and
    /// `{}` for empty containers, `\n` line ends and a final newline.
    pub fn to_json(&self) -> String {
        let mut out = String::new();
        self.json().write(&mut out, 0);
        out.push('\n');
        out
    }

    fn json(&self) -> Json {
        let members = |declaration: MemberDeclaration| {
            Json::Array(
                self.members_of(declaration)
                    .map(ModelMember::json)
                    .collect(),
            )
        };
        let mut top = BTreeMap::new();
        top.insert(
            "assembly".to_string(),
            self.assembly
                .as_ref()
                .map_or(Json::Null, ModelAssembly::json),
        );
        top.insert(
            "components".to_string(),
            members(MemberDeclaration::Component),
        );
        top.insert(
            "contracts".to_string(),
            members(MemberDeclaration::Contract),
        );
        top.insert(
            "edges".to_string(),
            Json::Array(self.edges.iter().map(Edge::json).collect()),
        );
        top.insert(
            "formatVersion".to_string(),
            Json::Str(FORMAT_VERSION.to_string()),
        );
        top.insert(
            "machines".to_string(),
            Json::Array(self.machines.iter().map(ModelMachine::json).collect()),
        );
        top.insert(
            "observations".to_string(),
            Json::Array(
                self.observations
                    .iter()
                    .map(ModelObservation::json)
                    .collect(),
            ),
        );
        top.insert("policies".to_string(), members(MemberDeclaration::Policy));
        top.insert(
            "resources".to_string(),
            members(MemberDeclaration::Resource),
        );
        top.insert(
            "sourceMap".to_string(),
            Json::Object(
                self.source_map
                    .iter()
                    .map(|(k, v)| (k.clone(), Json::Str(v.clone())))
                    .collect(),
            ),
        );
        top.insert("systemId".to_string(), Json::Str(self.system_id.clone()));
        Json::Object(top)
    }
}

impl ModelMember {
    fn json(&self) -> Json {
        let mut entry = BTreeMap::new();
        entry.insert(
            "declaration".to_string(),
            Json::Str(self.declaration.as_str().to_string()),
        );
        entry.insert(
            "defaults".to_string(),
            Json::Array(self.defaults.iter().map(|d| Json::Str(d.clone())).collect()),
        );
        entry.insert(
            "executable".to_string(),
            self.executable
                .as_ref()
                .map_or(Json::Null, |e| Json::Str(e.clone())),
        );
        entry.insert("id".to_string(), Json::Str(self.id.clone()));
        entry.insert("kind".to_string(), Json::Str(self.kind.clone()));
        entry.insert(
            "outputs".to_string(),
            Json::Object(
                self.outputs
                    .iter()
                    .map(|(field, ty)| (field.clone(), Json::Str(ty.clone())))
                    .collect(),
            ),
        );
        entry.insert("properties".to_string(), properties_json(&self.properties));
        Json::Object(entry)
    }
}

impl ModelAssembly {
    fn json(&self) -> Json {
        let mut entry = BTreeMap::new();
        entry.insert(
            "defaults".to_string(),
            Json::Array(self.defaults.iter().map(|d| Json::Str(d.clone())).collect()),
        );
        entry.insert("id".to_string(), Json::Str(self.id.clone()));
        entry.insert("kind".to_string(), Json::Str(self.kind.clone()));
        entry.insert("properties".to_string(), properties_json(&self.properties));
        entry.insert(
            "resolution".to_string(),
            match &self.resolution {
                Some(entries) => Json::Array(entries.iter().map(ResolutionEntry::json).collect()),
                None => Json::Null,
            },
        );
        entry.insert("system".to_string(), Json::Str(self.system.clone()));
        Json::Object(entry)
    }
}

impl ModelMachine {
    /// The model's entry: `disruption` only when declared (16.14).
    pub fn json(&self) -> Json {
        let mut entry = BTreeMap::new();
        if let Some(disruption) = &self.disruption {
            entry.insert("disruption".to_string(), Json::Str(disruption.clone()));
        }
        entry.insert("id".to_string(), Json::Str(self.id.clone()));
        entry.insert("machine".to_string(), value_json(&self.machine));
        entry.insert("member".to_string(), Json::Str(self.member.clone()));
        entry.insert(
            "subscribes".to_string(),
            Json::Array(
                self.subscribes
                    .iter()
                    .map(|s| Json::Str(s.clone()))
                    .collect(),
            ),
        );
        Json::Object(entry)
    }
}

impl ModelObservation {
    pub fn json(&self) -> Json {
        let mut entry = BTreeMap::new();
        entry.insert("element".to_string(), Json::Str(self.element.clone()));
        entry.insert("interval".to_string(), value_json(&self.interval));
        entry.insert("producer".to_string(), Json::Str(self.producer.clone()));
        entry.insert("ref".to_string(), Json::Str(self.reference.clone()));
        Json::Object(entry)
    }
}

impl ResolutionEntry {
    fn json(&self) -> Json {
        let mut entry = BTreeMap::new();
        entry.insert("binding".to_string(), value_json(&self.binding));
        entry.insert("member".to_string(), Json::Str(self.member.clone()));
        entry.insert("via".to_string(), Json::Str(self.via.clone()));
        Json::Object(entry)
    }
}

impl Edge {
    fn json(&self) -> Json {
        let mut entry = BTreeMap::new();
        entry.insert("from".to_string(), Json::Str(self.from.clone()));
        entry.insert(
            "kind".to_string(),
            Json::Str(self.kind.as_str().to_string()),
        );
        entry.insert("to".to_string(), Json::Str(self.to.clone()));
        Json::Object(entry)
    }
}

fn properties_json(properties: &[(String, Value)]) -> Json {
    Json::Object(
        properties
            .iter()
            .map(|(field, value)| (field.clone(), value_json(value)))
            .collect(),
    )
}

fn tagged(ty: &str, fields: Vec<(&str, Json)>) -> Json {
    let mut object: BTreeMap<String, Json> = fields
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    object.insert("type".to_string(), Json::Str(ty.to_string()));
    Json::Object(object)
}

/// The JSON of one value (`docs/slice.md` section 15.4).
pub fn value_json(value: &Value) -> Json {
    match value {
        Value::Unit => Json::Null,
        Value::Bool(b) => Json::Bool(*b),
        Value::Int(n) => Json::Int(*n),
        Value::Byte(b) => tagged("Byte", vec![("value", Json::Int(i64::from(*b)))]),
        Value::Text(text) => Json::Str(text.clone()),
        Value::List(items) => Json::Array(items.iter().map(value_json).collect()),
        Value::Tuple(items) => tagged(
            "Tuple",
            vec![("items", Json::Array(items.iter().map(value_json).collect()))],
        ),
        Value::Record { name, fields } => tagged(
            &name.to_string(),
            vec![(
                "fields",
                Json::Object(
                    fields
                        .iter()
                        .map(|(f, v)| (f.clone(), value_json(v)))
                        .collect(),
                ),
            )],
        ),
        Value::Enum {
            name,
            variant,
            fields,
        } => {
            let mut entries = vec![("variant", Json::Str(variant.clone()))];
            if !fields.is_empty() {
                entries.push((
                    "fields",
                    Json::Array(fields.iter().map(value_json).collect()),
                ));
            }
            tagged(&name.to_string(), entries)
        }
        Value::Map(entries) => tagged(
            "Map",
            vec![(
                "entries",
                Json::Array(
                    entries
                        .iter()
                        .map(|(k, v)| Json::Array(vec![value_json(k), value_json(v)]))
                        .collect(),
                ),
            )],
        ),
        Value::Quantity(kind, n) => {
            let unit = match kind {
                QuantityKind::Duration => "ns",
                QuantityKind::ByteSize => "bytes",
                QuantityKind::Ratio => "ppm",
            };
            tagged(kind.name(), vec![(unit, Json::Int(*n))])
        }
        Value::Reference(reference) => {
            let ty = match reference {
                Reference::System(_) => "SystemRef",
                Reference::Member { .. } => "Ref",
                Reference::Signal { .. } => "Signal",
                Reference::Protocol(_) => "Protocol",
            };
            tagged(ty, vec![("ref", Json::Str(reference.text()))])
        }
        Value::Member { system, member } => tagged(
            "Ref",
            vec![("ref", Json::Str(format!("{system}.{member}")))],
        ),
        Value::Module { module, fns } => tagged(
            "Module",
            vec![
                (
                    "fns",
                    Json::Object(
                        fns.iter()
                            .map(|(field, fn_name)| (field.clone(), Json::Str(fn_name.to_string())))
                            .collect(),
                    ),
                ),
                ("module", Json::Str(module.clone())),
            ],
        ),
        Value::Fn(name) => tagged("Fn", vec![("fn", Json::Str(name.to_string()))]),
        Value::Closure { module, offset, .. } => tagged(
            "Closure",
            vec![
                ("module", Json::Str(module.clone())),
                ("offset", Json::Int(*offset as i64)),
            ],
        ),
        Value::IoError { message, exit_code } => tagged(
            "IoError",
            vec![
                ("exitCode", Json::Int(*exit_code)),
                ("message", Json::Str(message.clone())),
            ],
        ),
    }
}

/// A JSON value in the canonical form of 15.5. Object keys are in byte
/// order (`BTreeMap<String, _>` orders `String`s by bytes).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Json {
    Null,
    Bool(bool),
    Int(i64),
    Str(String),
    Array(Vec<Json>),
    Object(BTreeMap<String, Json>),
}

impl Json {
    /// Append the canonical text at `indent` levels; no trailing newline.
    pub fn write(&self, out: &mut String, indent: usize) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(true) => out.push_str("true"),
            Json::Bool(false) => out.push_str("false"),
            Json::Int(n) => out.push_str(&n.to_string()),
            Json::Str(text) => write_string(out, text),
            Json::Array(items) if items.is_empty() => out.push_str("[]"),
            Json::Array(items) => {
                out.push_str("[\n");
                for (i, item) in items.iter().enumerate() {
                    pad(out, indent + 1);
                    item.write(out, indent + 1);
                    out.push_str(if i + 1 < items.len() { ",\n" } else { "\n" });
                }
                pad(out, indent);
                out.push(']');
            }
            Json::Object(entries) if entries.is_empty() => out.push_str("{}"),
            Json::Object(entries) => {
                out.push_str("{\n");
                for (i, (key, value)) in entries.iter().enumerate() {
                    pad(out, indent + 1);
                    write_string(out, key);
                    out.push_str(": ");
                    value.write(out, indent + 1);
                    out.push_str(if i + 1 < entries.len() { ",\n" } else { "\n" });
                }
                pad(out, indent);
                out.push('}');
            }
        }
    }
}

impl Json {
    /// The compact text: no whitespace, keys in byte order, the string
    /// escaping of [`Json::write`]. The form of a journal line (16.5).
    pub fn compact(&self) -> String {
        let mut out = String::new();
        self.write_compact(&mut out);
        out
    }

    fn write_compact(&self, out: &mut String) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(true) => out.push_str("true"),
            Json::Bool(false) => out.push_str("false"),
            Json::Int(n) => out.push_str(&n.to_string()),
            Json::Str(text) => write_string(out, text),
            Json::Array(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    item.write_compact(out);
                }
                out.push(']');
            }
            Json::Object(entries) => {
                out.push('{');
                for (i, (key, value)) in entries.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_string(out, key);
                    out.push(':');
                    value.write_compact(out);
                }
                out.push('}');
            }
        }
    }

    /// One JSON value from `text`, numbers as exact `i64` (15.4, O17);
    /// `Err` names the byte offset and what was expected. The inverse of
    /// [`Json::write`] and [`Json::compact`] for every text they produce.
    pub fn parse(text: &str) -> Result<Json, String> {
        let mut parser = Parser { text, pos: 0 };
        let value = parser.value()?;
        parser.space();
        if parser.pos != text.len() {
            return Err(format!(
                "byte {}: trailing characters after the value",
                parser.pos
            ));
        }
        Ok(value)
    }

    /// The value at `key` of an object.
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(entries) => entries.get(key),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(text) => Some(text),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Json::Int(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Json::Array(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&BTreeMap<String, Json>> {
        match self {
            Json::Object(entries) => Some(entries),
            _ => None,
        }
    }

    /// One word for the value's kind, for error texts.
    pub fn kind(&self) -> &'static str {
        match self {
            Json::Null => "null",
            Json::Bool(_) => "a boolean",
            Json::Int(_) => "an integer",
            Json::Str(_) => "a string",
            Json::Array(_) => "an array",
            Json::Object(_) => "an object",
        }
    }
}

/// A recursive-descent parser over `text`; `pos` is a byte offset.
struct Parser<'t> {
    text: &'t str,
    pos: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.text.as_bytes().get(self.pos).copied()
    }

    fn space(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\t' | b'\r')) {
            self.pos += 1;
        }
    }

    fn expect(&mut self, literal: &str) -> Result<(), String> {
        if self.text[self.pos..].starts_with(literal) {
            self.pos += literal.len();
            Ok(())
        } else {
            Err(format!("byte {}: expected `{literal}`", self.pos))
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        self.space();
        match self.peek() {
            None => Err(format!("byte {}: expected a value", self.pos)),
            Some(b'n') => self.expect("null").map(|()| Json::Null),
            Some(b't') => self.expect("true").map(|()| Json::Bool(true)),
            Some(b'f') => self.expect("false").map(|()| Json::Bool(false)),
            Some(b'"') => self.string().map(Json::Str),
            Some(b'[') => {
                self.pos += 1;
                let mut items = Vec::new();
                self.space();
                if self.peek() == Some(b']') {
                    self.pos += 1;
                    return Ok(Json::Array(items));
                }
                loop {
                    items.push(self.value()?);
                    self.space();
                    match self.peek() {
                        Some(b',') => self.pos += 1,
                        Some(b']') => {
                            self.pos += 1;
                            return Ok(Json::Array(items));
                        }
                        _ => return Err(format!("byte {}: expected `,` or `]`", self.pos)),
                    }
                }
            }
            Some(b'{') => {
                self.pos += 1;
                let mut entries = BTreeMap::new();
                self.space();
                if self.peek() == Some(b'}') {
                    self.pos += 1;
                    return Ok(Json::Object(entries));
                }
                loop {
                    self.space();
                    if self.peek() != Some(b'"') {
                        return Err(format!("byte {}: expected a key", self.pos));
                    }
                    let at = self.pos;
                    let key = self.string()?;
                    self.space();
                    self.expect(":")?;
                    let value = self.value()?;
                    if entries.insert(key.clone(), value).is_some() {
                        return Err(format!("byte {at}: duplicate key `{key}`"));
                    }
                    self.space();
                    match self.peek() {
                        Some(b',') => self.pos += 1,
                        Some(b'}') => {
                            self.pos += 1;
                            return Ok(Json::Object(entries));
                        }
                        _ => return Err(format!("byte {}: expected `,` or `}}`", self.pos)),
                    }
                }
            }
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(other) => Err(format!(
                "byte {}: unexpected `{}`",
                self.pos,
                char::from(other)
            )),
        }
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
        if matches!(self.peek(), Some(b'.' | b'e' | b'E')) {
            return Err(format!("byte {start}: a number must be an integer"));
        }
        let digits = &self.text[start..self.pos];
        digits
            .parse::<i64>()
            .map(Json::Int)
            .map_err(|_| format!("byte {start}: `{digits}` is not an i64"))
    }

    /// A string starting at the opening `"`.
    fn string(&mut self) -> Result<String, String> {
        self.pos += 1;
        let mut out = String::new();
        loop {
            let start = self.pos;
            loop {
                match self.peek() {
                    Some(b'"' | b'\\') | None => break,
                    Some(b) if b < 0x20 => {
                        return Err(format!("byte {}: control character in a string", self.pos));
                    }
                    Some(_) => self.pos += 1,
                }
            }
            out.push_str(&self.text[start..self.pos]);
            match self.peek() {
                None => return Err(format!("byte {}: unterminated string", self.pos)),
                Some(b'"') => {
                    self.pos += 1;
                    return Ok(out);
                }
                Some(_) => {
                    let at = self.pos;
                    self.pos += 1;
                    let Some(escape) = self.peek() else {
                        return Err(format!("byte {at}: unterminated escape"));
                    };
                    self.pos += 1;
                    match escape {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'n' => out.push('\n'),
                        b't' => out.push('\t'),
                        b'r' => out.push('\r'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'u' => {
                            let code = self.hex4()?;
                            let c = if (0xD800..0xDC00).contains(&code) {
                                self.expect("\\u")?;
                                let low = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&low) {
                                    return Err(format!("byte {at}: invalid surrogate pair"));
                                }
                                0x10000 + ((code - 0xD800) << 10) + (low - 0xDC00)
                            } else {
                                code
                            };
                            out.push(
                                char::from_u32(c)
                                    .ok_or_else(|| format!("byte {at}: invalid code point"))?,
                            );
                        }
                        other => {
                            return Err(format!(
                                "byte {at}: unknown escape `\\{}`",
                                char::from(other)
                            ));
                        }
                    }
                }
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let hex = self
            .text
            .get(self.pos..self.pos + 4)
            .ok_or_else(|| format!("byte {}: truncated `\\u` escape", self.pos))?;
        let code = u32::from_str_radix(hex, 16)
            .map_err(|_| format!("byte {}: `\\u{hex}` is not hexadecimal", self.pos))?;
        self.pos += 4;
        Ok(code)
    }
}

fn pad(out: &mut String, indent: usize) {
    for _ in 0..indent {
        out.push_str("  ");
    }
}

/// A JSON string: `"`, `\` and control characters escaped, everything
/// else as UTF-8 (15.5).
fn write_string(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// The model of the program `program` was typed from, with the assembly
/// named by `assembly` or the program's only one (15.6); E114 when none
/// or several are selectable, E110 to E113 from the resolution, E115,
/// E117 and E118 from the evaluation, each at its property expression in
/// the program file (a defaulted property's at its declaration).
pub fn model(
    evaluator: &mut Evaluator<'_>,
    program: &TypedProgram,
    assembly: Option<&str>,
) -> Result<Model, Vec<Diagnostic>> {
    let (system, assembly) = select(program, assembly)?;
    evaluator.with_stack(|evaluator| {
        let mut builder = Builder::new(evaluator, program);
        let members = builder.system(program, system);
        let assembly = assembly.and_then(|a| builder.assembly(program, a, Some(system)));
        if !builder.diagnostics.is_empty() {
            return Err(builder.diagnostics);
        }
        Ok(Model {
            system_id: system.name.clone(),
            members,
            assembly,
            edges: builder.edges.into_iter().collect(),
            source_map: builder.source_map,
            machines: builder.machines,
            observations: builder.observations,
        })
    })
}
