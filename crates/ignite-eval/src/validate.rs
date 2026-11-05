//! The check-time rules of slice 12 (`docs/slice.md` 16.17): E120 to E124
//! and E150 (22.3) per system and E121, E122, E129 and E151 per resolved
//! assembly (15.6), run by
//! `ignite check` for every system and assembly and by `ignite plan` and
//! `ignite run` for the selected one (O20). The compiler knows no kind, no
//! binding variant and no outcome or zone name (invariant 6): everything
//! here is found by name, as `admits` is (O18). The `validate` hook of a
//! kind decides E120 and its text is printed; the `entries` fn of the
//! binding type says which bindings run their member and what the kind
//! must declare for it; a producer's `interval` and `timeout` are its
//! `Duration` properties of those names (as `observations.interval`,
//! 16.14); a contract's window is its property of type `std.sli.Window`.
//! Owner: P1.

use std::collections::{BTreeMap, BTreeSet};

use ignite_syntax::ast::{Decl, Span, SystemDecl};
use ignite_syntax::diagnostic::Diagnostic;
use ignite_types::{
    AssemblyInfo, MemberInfo, QualName, QuantityKind, Reference, SystemInfo, Ty, TypedProgram,
};

use crate::model::{ModelMember, eval_diagnostic, type_name};
use crate::resolve::{binding_key_spans, entries_fn};
use crate::{Evaluator, Modules, ResolutionEntry, Value};

/// The record type's name of a kind, `None` for `TcpListener` and
/// `Store<T>`.
fn kind_name(ty: &Ty) -> Option<&QualName> {
    match ty {
        Ty::Record(q) | Ty::GenericRecord(q, _) => Some(q),
        _ => None,
    }
}

/// Whether `kind`'s module declares the associated fn `K.name`: the hooks
/// `validate` and `machine` (16.6).
fn declares_hook(modules: &Modules<'_>, kind: &QualName, name: &str) -> bool {
    let Some(module) = modules.get(&kind.module) else {
        return false;
    };
    module.program.decls.iter().any(|decl| match decl {
        Decl::Fn(f) => {
            !f.test_only
                && f.name.name == name
                && f.owner
                    .as_ref()
                    .is_some_and(|owner| owner.name == kind.name)
        }
        _ => false,
    })
}

/// `K.name` as the evaluator calls it.
fn hook(kind: &QualName, name: &str) -> QualName {
    QualName::new(kind.module.clone(), format!("{}.{name}", kind.name))
}

/// The members of the system by id, with their declaration info.
fn infos<'s>(
    system: &'s SystemInfo,
    members: &'s [ModelMember],
) -> Vec<(&'s ModelMember, &'s MemberInfo)> {
    members
        .iter()
        .filter_map(|member| {
            let name = member
                .id
                .strip_prefix(system.name.as_str())?
                .strip_prefix('.')?;
            Some((member, system.member(name)?))
        })
        .collect()
}

/// A member as the record value its kind's `validate` receives: every
/// field in the kind's order, an unset `Signal<T>` output as its own ref
/// (15.4).
fn member_value(
    program: &TypedProgram,
    system: &SystemInfo,
    info: &MemberInfo,
    member: &ModelMember,
    kind: &QualName,
) -> Value {
    let order: Vec<String> = match program.record_fields(kind) {
        Some(fields) => fields.iter().map(|(field, _)| field.clone()).collect(),
        None => member
            .properties
            .iter()
            .map(|(field, _)| field.clone())
            .chain(member.outputs.iter().map(|(field, _)| field.clone()))
            .collect(),
    };
    let fields = order
        .into_iter()
        .map(|field| {
            let value = match member.properties.iter().find(|(f, _)| *f == field) {
                Some((_, value)) => value.clone(),
                None => Value::Reference(Reference::Signal {
                    system: system.name.clone(),
                    member: info.name.clone(),
                    field: field.clone(),
                }),
            };
            (field, value)
        })
        .collect();
    Value::Record {
        name: kind.clone(),
        fields,
    }
}

/// The `Duration` property `field` of `member`, in base units.
fn duration(member: &ModelMember, field: &str) -> Option<i64> {
    match member.properties.iter().find(|(f, _)| f == field)? {
        (_, Value::Quantity(QuantityKind::Duration, value)) => Some(*value),
        _ => None,
    }
}

/// The literal text of a `Duration`.
fn duration_text(value: i64) -> String {
    Value::Quantity(QuantityKind::Duration, value).debug_text()
}

/// The signal `value` refers to, as `(system, member, field)`.
fn signal_ref(value: &Value) -> Option<(&str, &str, &str)> {
    match value {
        Value::Reference(Reference::Signal {
            system,
            member,
            field,
        }) => Some((system, member, field)),
        _ => None,
    }
}

/// The element type of the signal `producer.output` of `system`, when
/// `producer` is a member of `system` with that unset `Signal<T>` output.
fn element_of(
    system: &SystemInfo,
    signal_system: &str,
    producer: &str,
    output: &str,
) -> Option<Ty> {
    if signal_system != system.name {
        return None;
    }
    let (_, ty) = system
        .member(producer)?
        .outputs
        .iter()
        .find(|(o, _)| o == output)?;
    Some(match ty {
        Ty::Signal(element) => (**element).clone(),
        other => other.clone(),
    })
}

/// Whether the module of `kind` declares the hook `fn K.classify(self: K,
/// sample: element) -> Bool`, pure (`docs/slice.md` 22.3), with a generic
/// kind's type arguments put in for the hook's parameters.
fn classifies(modules: &Modules<'_>, kind: &Ty, element: &Ty) -> bool {
    let Some(q) = kind_name(kind) else {
        return false;
    };
    let Some(module) = modules.get(&q.module) else {
        return false;
    };
    let exports = module.types.exports();
    let Some(sig) = exports.fns.get(&format!("{}.classify", q.name)) else {
        return false;
    };
    if sig.params.len() != 2 || sig.ret != Ty::Bool || sig.effectful {
        return false;
    }
    let sample = match (&sig.params[0], kind) {
        (Ty::GenericRecord(_, params), Ty::GenericRecord(_, args)) => {
            let names: Vec<String> = params
                .iter()
                .filter_map(|p| match p {
                    Ty::Param(name) => Some(name.clone()),
                    _ => None,
                })
                .collect();
            sig.params[1].substitute(&names, args)
        }
        _ => sig.params[1].clone(),
    };
    sample == *element
}

/// Whether `ty` is an outcome type (`docs/slice.md` 22.3, E151):
/// `Bool`, `Int`, `Text`, `Byte`, a quantity, or a `List`,
/// `Option`, tuple, record or enum of outcome types. `seen` stops at a
/// record or enum already being checked (a recursive type).
fn outcome(ty: &Ty, modules: &Modules<'_>, seen: &mut Vec<QualName>) -> bool {
    match ty {
        Ty::Bool | Ty::Int | Ty::Text | Ty::Byte | Ty::Quantity(_) => true,
        Ty::List(item) | Ty::Option(item) => outcome(item, modules, seen),
        Ty::Tuple(items) => items.iter().all(|item| outcome(item, modules, seen)),
        Ty::Record(q) | Ty::GenericRecord(q, _) | Ty::Enum(q) | Ty::GenericEnum(q, _) => {
            if seen.contains(q) {
                return true;
            }
            let Some(module) = modules.get(&q.module) else {
                return false;
            };
            let types = module.types;
            let args: &[Ty] = match ty {
                Ty::GenericRecord(_, args) | Ty::GenericEnum(_, args) => args,
                _ => &[],
            };
            let params: Vec<String> = types
                .type_params(q)
                .map(<[String]>::to_vec)
                .unwrap_or_default();
            seen.push(q.clone());
            let ok = if let Some(fields) = types.record_fields(q) {
                fields
                    .iter()
                    .all(|(_, field)| outcome(&field.substitute(&params, args), modules, seen))
            } else if let Some(variants) = types.enum_variants(q) {
                variants.iter().all(|(_, payload)| {
                    payload
                        .iter()
                        .all(|item| outcome(&item.substitute(&params, args), modules, seen))
                })
            } else {
                false
            };
            seen.pop();
            ok
        }
        _ => false,
    }
}

/// The member `value` names, as `System.member`.
fn member_ref(value: &Value) -> Option<String> {
    match value {
        Value::Member { system, member }
        | Value::Reference(Reference::Member { system, member }) => {
            Some(format!("{system}.{member}"))
        }
        _ => None,
    }
}

/// Every member reference inside `value`, in order of appearance.
fn member_refs(value: &Value, out: &mut Vec<String>) {
    if let Some(id) = member_ref(value) {
        out.push(id);
        return;
    }
    match value {
        Value::List(items) | Value::Tuple(items) | Value::Enum { fields: items, .. } => {
            items.iter().for_each(|item| member_refs(item, out));
        }
        Value::Record { fields, .. } => fields.iter().for_each(|(_, v)| member_refs(v, out)),
        Value::Map(entries) => entries.iter().for_each(|(k, v)| {
            member_refs(k, out);
            member_refs(v, out);
        }),
        _ => {}
    }
}

/// The span of property `field` of `info` in the declaration, else the
/// declaration's start.
fn property_span(decl: Option<&SystemDecl>, info: &MemberInfo, field: &str) -> Span {
    decl.and_then(|decl| info.property(decl, field))
        .map_or(Span::new(info.offset, info.offset), |e| e.span)
}

/// Call `K.validate` on `value` and report each text it returns as E120
/// at `span`, for the declaration `path`.
fn validate_value(
    evaluator: &mut Evaluator<'_>,
    kind: &QualName,
    value: Value,
    path: &str,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match evaluator.call(&hook(kind, "validate"), vec![value]) {
        Ok(Value::List(items)) => {
            for item in items {
                if let Value::Text(reason) = item {
                    diagnostics.push(Diagnostic::new(
                        "E120",
                        span,
                        format!("declaration `{path}` of kind `{kind}` is refused: {reason}"),
                    ));
                }
            }
        }
        Ok(_) => {}
        Err(error) => diagnostics.push(eval_diagnostic(&error, path, span)),
    }
}

/// E120 for every record inside `value` whose type declares `validate`:
/// a `Restart { .. }` or `DrainRollout { .. }` set as a property is a
/// declaration of that kind too.
fn validate_nested(
    evaluator: &mut Evaluator<'_>,
    value: &Value,
    path: &str,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match value {
        Value::Record { name, fields } => {
            if declares_hook(evaluator.modules(), name, "validate") {
                validate_value(evaluator, name, value.clone(), path, span, diagnostics);
            }
            for (field, inner) in fields {
                validate_nested(
                    evaluator,
                    inner,
                    &format!("{path}.{field}"),
                    span,
                    diagnostics,
                );
            }
        }
        Value::List(items) | Value::Tuple(items) | Value::Enum { fields: items, .. } => {
            for item in items {
                validate_nested(evaluator, item, path, span, diagnostics);
            }
        }
        Value::Map(entries) => {
            for (_, inner) in entries {
                validate_nested(evaluator, inner, path, span, diagnostics);
            }
        }
        _ => {}
    }
}

/// E120, E123 and E124 for `system`, whose evaluated members are `members`
/// (`docs/slice.md` 16.17): every member whose kind's module declares
/// `K.validate` is validated as the record value of its declaration, and
/// so is every record value set in a property whose type declares
/// `validate`; a producer's `interval` and `timeout` must be positive with
/// `timeout < interval`; a `Rolling` window must be at least the interval
/// of the signal it reads. `Ok` means no diagnostic.
pub fn validate_system(
    evaluator: &mut Evaluator<'_>,
    program: &TypedProgram,
    system: &SystemInfo,
    members: &[ModelMember],
) -> Result<(), Vec<Diagnostic>> {
    let ast = evaluator
        .modules()
        .get(&program.module)
        .map(|module| module.program);
    let decl = ast.and_then(|ast| system.decl(ast));
    let mut diagnostics = Vec::new();
    let infos = infos(system, members);
    for (member, info) in &infos {
        let member_span = Span::new(info.offset, info.offset);
        if let Some(kind) = kind_name(&info.kind)
            && declares_hook(evaluator.modules(), kind, "validate")
        {
            let value = member_value(program, system, info, member, kind);
            validate_value(
                evaluator,
                kind,
                value,
                &member.id,
                member_span,
                &mut diagnostics,
            );
        }
        for (field, value) in &member.properties {
            let path = format!("{}.{field}", member.id);
            let span = property_span(decl, info, field);
            validate_nested(evaluator, value, &path, span, &mut diagnostics);
        }
        if !member.outputs.is_empty() {
            let interval = duration(member, "interval");
            let timeout = duration(member, "timeout");
            if let Some(interval) = interval
                && interval <= 0
            {
                diagnostics.push(Diagnostic::new(
                    "E123",
                    property_span(decl, info, "interval"),
                    format!(
                        "`{}.interval` is {}: a probe's interval must be positive",
                        member.id,
                        duration_text(interval)
                    ),
                ));
            }
            if let Some(timeout) = timeout
                && timeout <= 0
            {
                diagnostics.push(Diagnostic::new(
                    "E123",
                    property_span(decl, info, "timeout"),
                    format!(
                        "`{}.timeout` is {}: a probe's timeout must be positive",
                        member.id,
                        duration_text(timeout)
                    ),
                ));
            }
            if let (Some(interval), Some(timeout)) = (interval, timeout)
                && interval > 0
                && timeout > 0
                && timeout >= interval
            {
                diagnostics.push(Diagnostic::new(
                    "E123",
                    property_span(decl, info, "timeout"),
                    format!(
                        "`{}.timeout` ({}) must be below `{}.interval` ({}): probes of one member never overlap",
                        member.id,
                        duration_text(timeout),
                        member.id,
                        duration_text(interval)
                    ),
                ));
            }
        }
        for (field, value) in &member.properties {
            let Some((signal_system, producer, output)) = signal_ref(value) else {
                continue;
            };
            let producer_id = format!("{signal_system}.{producer}");
            let Some(interval) = members
                .iter()
                .find(|m| m.id == producer_id)
                .and_then(|m| duration(m, "interval"))
            else {
                continue;
            };
            for (window_field, window) in &member.properties {
                let Value::Enum {
                    name,
                    variant,
                    fields,
                } = window
                else {
                    continue;
                };
                if name.module != "std.sli" || name.name != "Window" || variant != "Rolling" {
                    continue;
                }
                let Some(Value::Quantity(QuantityKind::Duration, span_of_window)) = fields.first()
                else {
                    continue;
                };
                if *span_of_window < interval {
                    diagnostics.push(Diagnostic::new(
                        "E124",
                        property_span(decl, info, window_field),
                        format!(
                            "`{}.{window_field}` ({}) is shorter than the interval ({}) of its signal `{producer_id}.{output}` (`{}.{field}`), so it can hold no expected attempt",
                            member.id,
                            duration_text(*span_of_window),
                            duration_text(interval),
                            member.id
                        ),
                    ));
                }
            }
        }
        // E150: a member with a machine that subscribes to a signal and
        // whose kind cannot classify the signal's element (22.3).
        let Some(kind) = kind_name(&info.kind) else {
            continue;
        };
        if !declares_hook(evaluator.modules(), kind, "machine") {
            continue;
        }
        for (field, value) in &member.properties {
            let Some((signal_system, producer, output)) = signal_ref(value) else {
                continue;
            };
            let Some(element) = element_of(system, signal_system, producer, output) else {
                continue;
            };
            if classifies(evaluator.modules(), &info.kind, &element) {
                continue;
            }
            let element_text = type_name(&element);
            diagnostics.push(Diagnostic::new(
                "E150",
                property_span(decl, info, field),
                format!(
                    "{} `{}` of kind `{}` subscribes to `{signal_system}.{producer}.{output}` of `{element_text}` but declares no `fn {name}.classify(self: {name}, sample: {element_text}) -> Bool`",
                    info.declaration,
                    member.id,
                    type_name(&info.kind),
                    name = kind.name,
                ),
            ));
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// E121, E122 and E129 for `assembly` of `system`, whose evaluated members
/// are `members` and whose resolution is `resolution` (`docs/slice.md`
/// 16.17). The binding type's `entries` fn says which bindings run their
/// member (a non-empty list) and what the kind must declare; without it
/// nothing is checked. `Ok` means no diagnostic.
pub fn validate(
    evaluator: &mut Evaluator<'_>,
    program: &TypedProgram,
    system: &SystemInfo,
    members: &[ModelMember],
    assembly: &AssemblyInfo,
    resolution: &[ResolutionEntry],
) -> Result<(), Vec<Diagnostic>> {
    let Some(entries) = entries_fn(program, assembly, evaluator.modules()) else {
        return Ok(());
    };
    let ast = evaluator
        .modules()
        .get(&program.module)
        .map(|module| module.program);
    let decl = ast.and_then(|ast| system.decl(ast));
    let (key_spans, fallback) = binding_key_spans(evaluator, program, assembly);
    let span_of = |key: &str| key_spans.get(key).copied().unwrap_or(fallback);
    let mut diagnostics = Vec::new();
    // The entries each bindings key's binding needs.
    let mut needed: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for entry in resolution.iter().filter(|e| e.via == e.member) {
        let path = format!("{}.bindings[{}]", assembly.name, entry.member);
        match evaluator.call(&entries, vec![entry.binding.clone()]) {
            Ok(Value::List(items)) => {
                let names = items
                    .into_iter()
                    .filter_map(|item| match item {
                        Value::Text(name) => Some(name),
                        _ => None,
                    })
                    .collect();
                needed.insert(entry.member.clone(), names);
            }
            Ok(_) => {
                needed.insert(entry.member.clone(), Vec::new());
            }
            Err(error) => {
                diagnostics.push(eval_diagnostic(&error, &path, span_of(&entry.member)));
                needed.insert(entry.member.clone(), Vec::new());
            }
        }
    }
    let runs = |id: &str| {
        resolution
            .iter()
            .find(|e| e.member == id)
            .and_then(|e| needed.get(&e.via))
            .is_some_and(|names| !names.is_empty())
    };
    let infos = infos(system, members);
    // Signals already reported as E151.
    let mut undecodable: BTreeSet<String> = BTreeSet::new();
    // E129: a hosted member whose kind lacks an entry its host needs.
    for (id, names) in &needed {
        let Some((_, info)) = infos.iter().find(|(m, _)| m.id == *id) else {
            continue;
        };
        let kind = kind_name(&info.kind);
        for name in names {
            let declared = match name.as_str() {
                "main" => program
                    .executables()
                    .iter()
                    .any(|e| e.system == system.name && e.component == info.name),
                other => kind.is_some_and(|kind| declares_hook(evaluator.modules(), kind, other)),
            };
            if !declared {
                let kind_text = type_name(&info.kind);
                let fn_text = match kind {
                    Some(kind) => format!("{}.{name}", kind.name),
                    None => format!("{kind_text}.{name}"),
                };
                diagnostics.push(Diagnostic::new(
                    "E129",
                    span_of(id),
                    format!(
                        "`{id}` is hosted by a binding that runs it, but its kind `{kind_text}` declares no `{fn_text}`"
                    ),
                ));
            }
        }
    }
    for (member, info) in &infos {
        // E121: a signal with no producer the assembly runs.
        for (field, value) in &member.properties {
            let Some((signal_system, producer, output)) = signal_ref(value) else {
                continue;
            };
            let producer_id = format!("{signal_system}.{producer}");
            let span = property_span(decl, info, field);
            let is_output = signal_system == system.name
                && system
                    .member(producer)
                    .is_some_and(|p| p.outputs.iter().any(|(o, _)| o == output));
            if !is_output {
                diagnostics.push(Diagnostic::new(
                    "E121",
                    span,
                    format!(
                        "`{}.{field}` names `{producer_id}.{output}`, which is no output of `{producer_id}`",
                        member.id
                    ),
                ));
            } else if !runs(&producer_id) {
                diagnostics.push(Diagnostic::new(
                    "E121",
                    span,
                    format!(
                        "`{}.{field}` has no producer that assembly `{}` runs: `{producer_id}` is not bound to a host that runs it",
                        member.id, assembly.name
                    ),
                ));
            }
        }
        // E151: a signal the assembly runs and a machine subscribes to,
        // whose element type is no outcome type (22.3).
        if kind_name(&info.kind)
            .is_some_and(|kind| declares_hook(evaluator.modules(), kind, "machine"))
        {
            for (field, value) in &member.properties {
                let Some((signal_system, producer, output)) = signal_ref(value) else {
                    continue;
                };
                let producer_id = format!("{signal_system}.{producer}");
                if !runs(&producer_id) {
                    continue;
                }
                let Some(element) = element_of(system, signal_system, producer, output) else {
                    continue;
                };
                let signal = format!("{producer_id}.{output}");
                if outcome(&element, evaluator.modules(), &mut Vec::new())
                    || !undecodable.insert(signal.clone())
                {
                    continue;
                }
                diagnostics.push(Diagnostic::new(
                    "E151",
                    property_span(decl, info, field),
                    format!(
                        "signal `{signal}` carries `{}`; a probe outcome holds no fn, handle, reference or signal",
                        type_name(&element)
                    ),
                ));
            }
        }
        // E122: a run-time `Ref` that resolves to no endpoint or to several.
        if !runs(&member.id) {
            continue;
        }
        let Some(kind) = kind_name(&info.kind) else {
            continue;
        };
        let Some(fields) = program.record_fields(kind) else {
            continue;
        };
        for (field, ty) in fields {
            if !matches!(ty, Ty::Ref(_)) {
                continue;
            }
            let Some(target) = member
                .properties
                .iter()
                .find(|(f, _)| f == field)
                .and_then(|(_, v)| member_ref(v))
            else {
                continue;
            };
            let span = property_span(decl, info, field);
            if !runs(&target) {
                diagnostics.push(Diagnostic::new(
                    "E122",
                    span,
                    format!(
                        "`{}.{field}` names `{target}`, which assembly `{}` does not run as a process",
                        member.id, assembly.name
                    ),
                ));
                continue;
            }
            let mut referenced = Vec::new();
            if let Some((target_member, _)) = infos.iter().find(|(m, _)| m.id == target) {
                for (_, value) in &target_member.properties {
                    member_refs(value, &mut referenced);
                }
            }
            let listeners: Vec<&str> = members
                .iter()
                .filter(|m| m.kind == "TcpListener" && referenced.contains(&m.id))
                .map(|m| m.id.as_str())
                .collect();
            match listeners.len() {
                1 => {}
                0 => diagnostics.push(Diagnostic::new(
                    "E122",
                    span,
                    format!(
                        "`{}.{field}` names `{target}`, which binds no `TcpListener` resource: `Endpoint.of` resolves to no endpoint",
                        member.id
                    ),
                )),
                n => diagnostics.push(Diagnostic::new(
                    "E122",
                    span,
                    format!(
                        "`{}.{field}` names `{target}`, which binds {n} `TcpListener` resources ({}): `Endpoint.of` resolves to several endpoints",
                        member.id,
                        listeners.join(", ")
                    ),
                )),
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}
