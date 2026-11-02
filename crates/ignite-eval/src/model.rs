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

use ignite_syntax::ast::{Expr, ExprKind, Span, SystemDecl};
use ignite_syntax::diagnostic::Diagnostic;
use ignite_types::{
    AssemblyInfo, ExecutableInfo, MemberDeclaration, MemberInfo, PropertySource, QualName,
    QuantityKind, Reference, SystemInfo, TCP_LISTENER_DEFAULT_BACKLOG,
    TCP_LISTENER_DEFAULT_EXPOSURE, Ty, TypedProgram, field_default,
};

use crate::eval::Limit;
use crate::resolve::binding_fns;
use crate::{EvalError, Evaluator, Module, ResolutionEntry, Value, resolve};

/// The `formatVersion` of the model (O13).
pub const FORMAT_VERSION: &str = "ignite-model/0.1";

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
}

impl EdgeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EdgeKind::Uses => "uses",
            EdgeKind::References => "references",
            EdgeKind::Observes => "observes",
            EdgeKind::Hosts => "hosts",
        }
    }

    /// The passive form `explain` prints for an incoming edge.
    pub fn passive(self) -> &'static str {
        match self {
            EdgeKind::Uses => "used by",
            EdgeKind::References => "referenced by",
            EdgeKind::Observes => "observed by",
            EdgeKind::Hosts => "hosted by",
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
    })
}
