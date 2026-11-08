//! `ignite explain` (`docs/slice.md` section 15.8): one system, member,
//! property path or assembly of a model as deterministic plain text.
//! Owner: P3.
//!
//! Values print as slice-9 debug text (13.6) with the 15.8 changes:
//! quantities in their literal form (14.1), `Ref`, `Signal`, `SystemRef`
//! and `Protocol` as their name unquoted, enum variants without the enum
//! name (`Rolling(30 days)`, `WorkstationExecutable`, as the goldens of
//! 15.8 show), `module("p")` for a module record and a fn by its
//! qualified name.
//!
//! A machine id (`Terminal.server.rollout`, 16.14) is explained before a
//! property path of the same text: the machine's state fields and its
//! `disruption` (16.12); the property stays visible in its member's
//! explanation.

use ignite_syntax::ast::Span;
use ignite_syntax::diagnostic::Diagnostic;
use ignite_types::MemberDeclaration;

use crate::{Model, ModelAssembly, ModelMachine, ModelMember, Value};

/// The explanation of `path` (`Terminal`, `Terminal.reachable`,
/// `Terminal.server.recovery`, `Terminal.server.supervisor`, `local`) in
/// `model`; E119 when the path names none of these.
pub fn explain(model: &Model, path: &str) -> Result<String, Diagnostic> {
    if path == model.system_id {
        return Ok(system(model));
    }
    if let Some(assembly) = model.assembly.as_ref().filter(|a| a.id == path) {
        return Ok(assembly_text(model, assembly));
    }
    if let Some(machine) = model.machines.iter().find(|m| m.id == path) {
        return Ok(machine_text(machine));
    }
    if let Some(member) = model.member(path) {
        return Ok(member_text(model, member));
    }
    if let Some((member, rest)) = model.members.iter().find_map(|m| {
        path.strip_prefix(m.id.as_str())?
            .strip_prefix('.')
            .map(|r| (m, r))
    }) && let Some(text) = property_text(model, member, path, rest)
    {
        return Ok(text);
    }
    Err(Diagnostic::new(
        "E119",
        Span::new(0, 0),
        format!("`{path}` names no system, member, property or assembly of this program"),
    ))
}

fn position<'m>(model: &'m Model, key: &str) -> &'m str {
    model.source_map.get(key).map_or("", String::as_str)
}

/// `Terminal: system (file:line:column)` and one line per member.
fn system(model: &Model) -> String {
    let mut out = format!(
        "{}: system ({})\n",
        model.system_id,
        position(model, &model.system_id)
    );
    for member in &model.members {
        out.push_str(&format!(
            "  {} {}: {}",
            member.declaration, member.id, member.kind
        ));
        if let Some(executable) = &member.executable {
            out.push_str(&format!(", executable {executable}"));
        }
        out.push('\n');
    }
    if let Some(assembly) = &model.assembly {
        out.push_str(&format!("assembly {}: {}\n", assembly.id, assembly.kind));
    }
    out
}

/// The member form of 15.8: declaration, kind and position; properties in
/// the kind's field order with ` (default)` after a defaulted one; outputs;
/// edges out of and into the member; the resolution line when an assembly
/// is selected; `standing` for a contract.
fn member_text(model: &Model, member: &ModelMember) -> String {
    let mut out = format!(
        "{}: {} {} ({})\n",
        member.id,
        member.declaration,
        member.kind,
        position(model, &member.id)
    );
    for (field, value) in &member.properties {
        out.push_str(&format!("  {field} = {}", render(value)));
        if member.defaults.contains(field) {
            out.push_str(" (default)");
        }
        out.push('\n');
    }
    for (field, ty) in &member.outputs {
        out.push_str(&format!("  {field}: output of {ty}\n"));
    }
    for edge in &model.edges {
        if edge.from == member.id {
            out.push_str(&format!("{} {}\n", edge.kind.as_str(), edge.to));
        }
    }
    let prefix = format!("{}.", member.id);
    for edge in &model.edges {
        if edge.to == member.id || edge.to.starts_with(&prefix) {
            out.push_str(&format!("{} {}\n", edge.kind.passive(), edge.from));
        }
    }
    if let Some(entry) = model
        .assembly
        .as_ref()
        .and_then(|a| a.resolution.as_ref())
        .and_then(|entries| entries.iter().find(|e| e.member == member.id))
    {
        out.push_str(&format!("resolution: {}", render(&entry.binding)));
        if entry.via != entry.member {
            out.push_str(&format!(" via {}", entry.via));
        }
        out.push('\n');
    }
    for machine in model.machines.iter().filter(|m| m.member == member.id) {
        out.push_str(&format!("machine {}: {}\n", machine.id, machine.variant()));
    }
    if member.declaration == MemberDeclaration::Contract {
        out.push_str("standing: evaluated at run time (ignite run, ignite replay)\n");
    }
    out
}

/// The machine form of 16.14: variant and member, the state fields of the
/// payload in declaration order, the subscribed signals and the declared
/// disruption (16.12).
fn machine_text(machine: &ModelMachine) -> String {
    let mut out = format!(
        "{}: machine {} of {}\n",
        machine.id,
        machine.variant(),
        machine.member
    );
    if let Some(Value::Record { fields, .. }) = machine.payload() {
        for (field, value) in fields {
            out.push_str(&format!("  {field} = {}\n", render(value)));
        }
    }
    for signal in &machine.subscribes {
        out.push_str(&format!("subscribes {signal}\n"));
    }
    if let Some(disruption) = &machine.disruption {
        out.push_str(&format!("disruption: {disruption}\n"));
    }
    out
}

/// A property path below `member`: `rest` is `field` or
/// `field.subfield...`, walked through record fields, map keys written as
/// text and list indices. A record value lists its fields, any other
/// value one `value =` line. `None` when the path names nothing.
fn property_text(model: &Model, member: &ModelMember, path: &str, rest: &str) -> Option<String> {
    let mut segments = rest.split('.');
    let field = segments.next()?;
    let mut value: &Value = &member.properties.iter().find(|(f, _)| f == field)?.1;
    for segment in segments {
        value = match value {
            Value::Record { fields, .. } => &fields.iter().find(|(f, _)| f == segment)?.1,
            Value::Map(entries) => {
                &entries
                    .iter()
                    .find(|(k, _)| matches!(k, Value::Text(t) if t == segment))?
                    .1
            }
            Value::List(items) => items.get(segment.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    let defaulted = member.defaults.iter().any(|d| d == field);
    let mut out = format!("{path}: property {}", value_type(value));
    if defaulted {
        out.push_str(" (default)");
    }
    let position_key = format!("{}.{field}", member.id);
    out.push_str(&format!(" ({})\n", position(model, &position_key)));
    match value {
        Value::Record { fields, .. } => {
            for (field, value) in fields {
                out.push_str(&format!("  {field} = {}\n", render(value)));
            }
        }
        other => out.push_str(&format!("  value = {}\n", render(other))),
    }
    Some(out)
}

/// The assembly form of 15.8: the resolution, one line per member in id
/// order, `via` for a hosted member and the executable of an executable
/// component; the evaluated properties when the kind is not resolvable.
fn assembly_text(model: &Model, assembly: &ModelAssembly) -> String {
    let mut out = format!(
        "{}: assembly {} of {} ({})\n",
        assembly.id,
        assembly.kind,
        assembly.system,
        position(model, &assembly.id)
    );
    match &assembly.resolution {
        Some(entries) => {
            for entry in entries {
                out.push_str(&format!("  {}", entry.member));
                if entry.via != entry.member {
                    out.push_str(&format!(" via {}", entry.via));
                }
                out.push_str(&format!(" = {}", render(&entry.binding)));
                if let Some(executable) = model
                    .member(&entry.member)
                    .and_then(|m| m.executable.as_ref())
                {
                    out.push_str(&format!(", executable {executable}"));
                }
                out.push('\n');
            }
        }
        None => {
            for (field, value) in &assembly.properties {
                out.push_str(&format!("  {field} = {}", render(value)));
                if assembly.defaults.contains(field) {
                    out.push_str(" (default)");
                }
                out.push('\n');
            }
        }
    }
    out
}

/// The type a property explanation names for `value`.
fn value_type(value: &Value) -> String {
    match value {
        Value::Unit => "Unit".to_string(),
        Value::Bool(_) => "Bool".to_string(),
        Value::Int(_) => "Int".to_string(),
        Value::Byte(_) => "Byte".to_string(),
        Value::U32(_) => "U32".to_string(),
        Value::Text(_) => "Text".to_string(),
        Value::List(_) => "List".to_string(),
        Value::Tuple(_) => "Tuple".to_string(),
        Value::Record { name, .. } | Value::Enum { name, .. } => name.to_string(),
        Value::Map(_) => "Map".to_string(),
        Value::Quantity(kind, _) => kind.name().to_string(),
        Value::Reference(ignite_types::Reference::System(_)) => "SystemRef".to_string(),
        Value::Reference(ignite_types::Reference::Signal { .. }) => "Signal".to_string(),
        Value::Reference(ignite_types::Reference::Protocol(_)) => "Protocol".to_string(),
        Value::Reference(_) | Value::Member { .. } => "Ref".to_string(),
        Value::Module { .. } => "Module".to_string(),
        Value::Fn(_) => "Fn".to_string(),
        Value::Closure { .. } => "Closure".to_string(),
        Value::IoError { .. } => "IoError".to_string(),
    }
}

/// The text of a value in an explanation (module doc).
pub fn render(value: &Value) -> String {
    let list = |items: &[Value]| items.iter().map(render).collect::<Vec<_>>().join(", ");
    match value {
        Value::Unit => "()".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Int(n) => n.to_string(),
        Value::Byte(b) => b.to_string(),
        Value::U32(x) => x.to_string(),
        Value::Text(text) => quote(text),
        Value::List(items) => format!("[{}]", list(items)),
        Value::Tuple(items) => format!("({})", list(items)),
        Value::Record { name, fields } if fields.is_empty() => format!("{} {{}}", name.name),
        Value::Record { name, fields } => format!(
            "{} {{ {} }}",
            name.name,
            fields
                .iter()
                .map(|(f, v)| format!("{f}: {}", render(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Enum {
            variant, fields, ..
        } if fields.is_empty() => variant.clone(),
        Value::Enum {
            variant, fields, ..
        } => format!("{variant}({})", list(fields)),
        Value::Map(entries) => format!(
            "{{{}}}",
            entries
                .iter()
                .map(|(k, v)| format!("{}: {}", render(k), render(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Quantity(kind, n) => kind.debug_text(*n),
        Value::Reference(reference) => reference.text(),
        Value::Member { system, member } => format!("{system}.{member}"),
        Value::Module { module, .. } => format!("module({})", quote(module)),
        Value::Fn(name) => name.to_string(),
        Value::Closure { .. } => "<closure>".to_string(),
        Value::IoError { message, .. } => format!("IoError({})", quote(message)),
    }
}

/// A `Text` in the debug form of 13.6.
fn quote(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\0' => out.push_str("\\0"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                out.push_str(&format!("\\u{{{:x}}}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
