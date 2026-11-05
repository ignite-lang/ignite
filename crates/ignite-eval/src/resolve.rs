//! Assembly resolution (`docs/slice.md` section 15.6): which binding hosts
//! each resource and component, decided by the `admits` and `includes`
//! fns of the binding type's module (O18), with E110 to E113. Owner: P3.

use std::collections::BTreeMap;

use ignite_syntax::ast::{Decl, ExprKind, Span};
use ignite_syntax::diagnostic::Diagnostic;
use ignite_types::{
    AssemblyInfo, MemberDeclaration, MemberInfo, QualName, Reference, SystemInfo, Ty, TypedProgram,
};

use crate::model::{eval_diagnostic, type_name};
use crate::{Evaluator, Modules, Value};

/// One entry of `assembly.resolution` (15.6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolutionEntry {
    /// The member's id, `Terminal.console`.
    pub member: String,
    /// The key whose binding hosts the member: the member itself when it is
    /// a key.
    pub via: String,
    /// The binding's evaluated value.
    pub binding: Value,
}

/// The `admits` and `includes` fns that make `assembly`'s kind resolvable
/// (15.6, O18): the kind has a field `bindings: Map<Ref, B>` and `B`'s
/// module declares `fn admits(binding: B, member: Member) -> Option<Text>`
/// and `fn includes(binding: B) -> List<Ref>`. `None` for any other kind.
pub fn binding_fns(
    program: &TypedProgram,
    assembly: &AssemblyInfo,
    modules: &Modules<'_>,
) -> Option<(QualName, QualName)> {
    let binding = binding_type(program, assembly)?;
    let module = modules.get(&binding.module)?;
    if !declares_free_fn(module.program, "admits", 2)
        || !declares_free_fn(module.program, "includes", 1)
    {
        return None;
    }
    Some((
        QualName::new(binding.module.clone(), "admits"),
        QualName::new(binding.module.clone(), "includes"),
    ))
}

/// The `entries` fn of `assembly`'s binding type (`docs/slice.md` 16.17,
/// O18): `fn entries(binding: B) -> List<Text>`, the fns the hosted kind
/// must declare for the binding to run it (`main`, `machine`). A binding
/// with a non-empty list runs its member; E121, E122 and E129 rest on it.
/// `None` when the module declares no such fn, and the three codes are
/// not checked.
pub fn entries_fn(
    program: &TypedProgram,
    assembly: &AssemblyInfo,
    modules: &Modules<'_>,
) -> Option<QualName> {
    let binding = binding_type(program, assembly)?;
    let module = modules.get(&binding.module)?;
    declares_free_fn(module.program, "entries", 1)
        .then(|| QualName::new(binding.module.clone(), "entries"))
}

/// The binding type `B` of an assembly kind with a field
/// `bindings: Map<Ref, B>`.
fn binding_type<'t>(program: &'t TypedProgram, assembly: &AssemblyInfo) -> Option<&'t QualName> {
    let kind = match &assembly.kind {
        Ty::Record(q) | Ty::GenericRecord(q, _) => q,
        _ => return None,
    };
    let fields = program.record_fields(kind)?;
    let (_, ty) = fields.iter().find(|(field, _)| field == "bindings")?;
    let Ty::Map(key, binding) = ty else {
        return None;
    };
    if !matches!(**key, Ty::Ref(_)) {
        return None;
    }
    match &**binding {
        Ty::Enum(q) | Ty::Record(q) | Ty::GenericEnum(q, _) | Ty::GenericRecord(q, _) => Some(q),
        _ => None,
    }
}

/// Whether `program` declares a free, non-test fn `name` with `arity`
/// parameters.
fn declares_free_fn(program: &ignite_syntax::ast::Program, name: &str, arity: usize) -> bool {
    program.decls.iter().any(|decl| match decl {
        Decl::Fn(f) => {
            f.name.name == name && f.owner.is_none() && !f.test_only && f.params.len() == arity
        }
        _ => false,
    })
}

/// The span of each key of `assembly`'s `bindings` literal by member id,
/// and the fallback span (the `bindings` property, else the assembly) for
/// a key that has none.
pub(crate) fn binding_key_spans(
    evaluator: &Evaluator<'_>,
    program: &TypedProgram,
    assembly: &AssemblyInfo,
) -> (BTreeMap<String, Span>, Span) {
    let ast = evaluator
        .modules()
        .get(&program.module)
        .map(|module| module.program);
    let bindings_expr = ast.and_then(|ast| assembly.property(ast, "bindings"));
    let fallback = bindings_expr.map_or(Span::new(assembly.offset, assembly.offset), |e| e.span);
    let key_spans = match bindings_expr.map(|e| &e.kind) {
        Some(ExprKind::Brace { entries }) => entries
            .iter()
            .map(|entry| (entry.key.dotted(), entry.key.span))
            .collect(),
        _ => BTreeMap::new(),
    };
    (key_spans, fallback)
}

/// The `std.deploy.Member` record `admits` is asked about (15.6).
fn member_value(member: &MemberInfo, executable: bool) -> Value {
    Value::Record {
        name: QualName::new("std.deploy", "Member"),
        fields: vec![
            (
                "declaration".to_string(),
                Value::Text(member.declaration.as_str().to_string()),
            ),
            ("kind".to_string(), Value::Text(type_name(&member.kind))),
            ("executable".to_string(), Value::Bool(executable)),
            (
                "outputs".to_string(),
                Value::List(
                    member
                        .outputs
                        .iter()
                        .map(|(field, _)| Value::Text(field.clone()))
                        .collect(),
                ),
            ),
        ],
    }
}

/// The member id a map key or an `includes` item denotes.
fn member_id(value: &Value) -> Option<String> {
    match value {
        Value::Reference(reference @ Reference::Member { .. }) => Some(reference.text()),
        Value::Member { system, member } => Some(format!("{system}.{member}")),
        _ => None,
    }
}

/// Resolve `assembly` of `system`, whose evaluated `bindings` map is
/// `bindings`: one entry per resource and component, sorted by member id,
/// or the E110 to E113 diagnostics of the rules table in 15.6. E111 to
/// E113 are reported at the key's entry in the `bindings` literal, E110 at
/// the `bindings` property.
pub fn resolve(
    evaluator: &mut Evaluator<'_>,
    program: &TypedProgram,
    system: &SystemInfo,
    assembly: &AssemblyInfo,
    bindings: &Value,
) -> Result<Vec<ResolutionEntry>, Vec<Diagnostic>> {
    let Some((admits, includes)) = binding_fns(program, assembly, evaluator.modules()) else {
        return Ok(Vec::new());
    };
    let Value::Map(entries) = bindings else {
        return Ok(Vec::new());
    };
    let (key_spans, fallback) = binding_key_spans(evaluator, program, assembly);
    let span_of = |key: &str| key_spans.get(key).copied().unwrap_or(fallback);
    let mut diagnostics = Vec::new();
    // Key id to its binding, in key order.
    let mut keys: Vec<(String, Value)> = Vec::new();
    // Included member id to the key that includes it.
    let mut included: BTreeMap<String, String> = BTreeMap::new();
    for (key, binding) in entries {
        let Some(id) = member_id(key) else {
            continue;
        };
        let Some(member) = id
            .strip_prefix(system.name.as_str())
            .and_then(|rest| rest.strip_prefix('.'))
            .and_then(|name| system.member(name))
        else {
            continue;
        };
        let span = span_of(&id);
        if matches!(
            member.declaration,
            MemberDeclaration::Contract | MemberDeclaration::Policy
        ) {
            diagnostics.push(Diagnostic::new(
                "E112",
                span,
                format!(
                    "`{id}` is a {}: contracts and policies are never bindings keys",
                    member.declaration
                ),
            ));
            continue;
        }
        let executable = program
            .executables()
            .iter()
            .find(|e| e.system == system.name && e.component == member.name);
        let path = format!("{}.bindings[{id}]", assembly.name);
        match evaluator.call(
            &admits,
            vec![binding.clone(), member_value(member, executable.is_some())],
        ) {
            Ok(Value::Enum {
                variant, fields, ..
            }) if variant == "Some" => {
                let reason = match fields.first() {
                    Some(Value::Text(reason)) => reason.clone(),
                    _ => "not admitted".to_string(),
                };
                diagnostics.push(Diagnostic::new(
                    "E112",
                    span,
                    format!("binding of `{id}` is not admitted: {reason}"),
                ));
            }
            Ok(_) => {}
            Err(error) => diagnostics.push(eval_diagnostic(&error, &path, span)),
        }
        let hosted: Vec<String> = match evaluator.call(&includes, vec![binding.clone()]) {
            Ok(Value::List(items)) => items.iter().filter_map(member_id).collect(),
            Ok(_) => Vec::new(),
            Err(error) => {
                diagnostics.push(eval_diagnostic(&error, &path, span));
                Vec::new()
            }
        };
        let embedded: Vec<String> = executable
            .map(|e| {
                e.embedded
                    .iter()
                    .map(|(name, _)| format!("{}.{name}", system.name))
                    .collect()
            })
            .unwrap_or_default();
        for other in &hosted {
            if !embedded.contains(other) {
                diagnostics.push(Diagnostic::new(
                    "E113",
                    span,
                    format!("binding of `{id}` includes `{other}`, which `{id}` does not embed"),
                ));
            }
            match included.get(other) {
                Some(first) => diagnostics.push(Diagnostic::new(
                    "E111",
                    span,
                    format!("`{other}` is included by both `{first}` and `{id}`"),
                )),
                None => {
                    included.insert(other.clone(), id.clone());
                }
            }
        }
        for other in &embedded {
            if !hosted.contains(other) {
                diagnostics.push(Diagnostic::new(
                    "E113",
                    span,
                    format!("binding of `{id}` does not include `{other}`, which `{id}` embeds"),
                ));
            }
        }
        keys.push((id, binding.clone()));
    }
    for (id, _) in &keys {
        if let Some(by) = included.get(id) {
            diagnostics.push(Diagnostic::new(
                "E111",
                span_of(id),
                format!("`{id}` is both a bindings key and included by `{by}`"),
            ));
        }
    }
    // The key whose binding hosts `id` and that binding, for a key or an
    // included member.
    let host = |id: &str| -> Option<(String, Value)> {
        let via = if keys.iter().any(|(key, _)| key == id) {
            id
        } else {
            included.get(id)?.as_str()
        };
        let binding = keys
            .iter()
            .find(|(key, _)| key == via)
            .map(|(_, binding)| binding.clone())
            .unwrap_or(Value::Unit);
        Some((via.to_string(), binding))
    };
    let mut resolution = Vec::new();
    for member in &system.members {
        if !matches!(
            member.declaration,
            MemberDeclaration::Resource | MemberDeclaration::Component
        ) {
            continue;
        }
        if let Some((via, binding)) = host(&member.id) {
            resolution.push(ResolutionEntry {
                member: member.id.clone(),
                via,
                binding,
            });
        } else if let Some((via, binding)) = implicit_host(program, system, member, &host) {
            resolution.push(ResolutionEntry {
                member: member.id.clone(),
                via,
                binding,
            });
        } else {
            diagnostics.push(Diagnostic::new(
                "E110",
                fallback,
                format!(
                    "`{}` is neither a bindings key nor included by a binding of assembly `{}`",
                    member.id, assembly.name
                ),
            ));
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    resolution.sort_by(|a, b| a.member.cmp(&b.member));
    Ok(resolution)
}

/// The host of a capability resource that is no bindings key and that no
/// binding includes (15.6): the process of the first declared executable
/// component that creates it (its `ExecutableInfo::resources`, embedded
/// components' included), hosted as that component is. `None` for any
/// other member, and for a capability no hosted executable creates.
fn implicit_host(
    program: &TypedProgram,
    system: &SystemInfo,
    member: &MemberInfo,
    host: &dyn Fn(&str) -> Option<(String, Value)>,
) -> Option<(String, Value)> {
    if member.declaration != MemberDeclaration::Resource
        || !matches!(member.kind, Ty::Capability(_))
    {
        return None;
    }
    system
        .members
        .iter()
        .filter(|component| component.declaration == MemberDeclaration::Component)
        .filter(|component| {
            program.executables().iter().any(|e| {
                e.system == system.name
                    && e.component == component.name
                    && e.resources.iter().any(|(name, _)| *name == member.name)
            })
        })
        .find_map(|component| host(&component.id))
}
