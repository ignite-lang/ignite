//! Values of the evaluator (`docs/slice.md` sections 15.3 and 15.4) and
//! their debug text (section 13.6, 14.1 quantities in literal form). The
//! model encodes a [`Value`] as the JSON of the 15.4 table. Owner: P2.

use std::collections::BTreeMap;

use ignite_types::{QualName, QuantityKind, Reference, Ty, TypedProgram};

use crate::model::Json;

/// Why a JSON text is no value of the type it was decoded as
/// (`docs/slice.md` 16.16, the inverse of 15.4): the first part that does
/// not fit, for the probe adapter's `AttemptFailed` reason and E127.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodeError {
    pub message: String,
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for DecodeError {}

/// One Ignite value. Records and enums carry the qualified name of their
/// type as the typer prints it (`std.policy.Restart`); the builtin enums
/// `Option`, `Result`, `Input` and `Exposure` carry [`QualName::builtin`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Unit,
    Bool(bool),
    Int(i64),
    Byte(u8),
    Text(String),
    /// `Bytes` is a list of bytes.
    List(Vec<Value>),
    /// Two or more items.
    Tuple(Vec<Value>),
    /// Fields in the record's declaration order, every field present.
    Record {
        name: QualName,
        fields: Vec<(String, Value)>,
    },
    /// A variant with its payload values.
    Enum {
        name: QualName,
        variant: String,
        fields: Vec<Value>,
    },
    /// Entries in key order (`docs/slice.md` section 14.3).
    Map(Vec<(Value, Value)>),
    /// `Duration`, `ByteSize` or `Ratio` in base units.
    Quantity(QuantityKind, i64),
    /// A `Ref`, `Signal`, `SystemRef` or `Protocol` value.
    Reference(Reference),
    /// A bare member name used as a value (`listener = ingress`,
    /// `application = console`): the model shows the member's `Ref` and a
    /// `uses` edge (15.4), the embedded value being the member's own entry.
    Member {
        system: String,
        member: String,
    },
    /// `module("p")`: the module path and the fn that fills each field, in
    /// field order.
    Module {
        module: String,
        fns: Vec<(String, QualName)>,
    },
    /// A named top-level fn used as a value.
    Fn(QualName),
    /// A closure, by the module and byte offset of its literal, with the
    /// values it captured at creation (`docs/slice.md` section 13.3, by
    /// copy); E118 when it reaches the model.
    Closure {
        module: String,
        offset: usize,
        captures: Vec<(String, Value)>,
    },
    /// An `IoError` built by `IoError.new` (exit code 1) or
    /// `with_exit_code`; E118 when it reaches the model.
    IoError {
        message: String,
        exit_code: i64,
    },
}

impl Value {
    /// The value the JSON text `json` encodes as a value of type `ty` in
    /// the encoding of `docs/slice.md` 15.4 (16.16): the driver decodes a
    /// probe's stdout line by the signal's element type, and replay
    /// decodes the journal's `event` records. `types` is any typed program
    /// that was typed against the modules declaring the records and enums
    /// in `ty` (it gives the field order and the payload types, which the
    /// JSON does not carry). A text that is no such value is `Err`, naming
    /// the first part that does not fit.
    pub fn decode(json: &str, ty: &Ty, types: &TypedProgram) -> Result<Value, DecodeError> {
        let parsed = Json::parse(json).map_err(|message| DecodeError { message })?;
        Value::decode_json(&parsed, ty, types)
    }

    /// [`Value::decode`] of a parsed JSON value.
    pub fn decode_json(json: &Json, ty: &Ty, types: &TypedProgram) -> Result<Value, DecodeError> {
        decode(json, ty, types, "value")
    }

    /// `Some(v)`.
    pub fn some(value: Value) -> Value {
        Value::Enum {
            name: QualName::builtin("Option"),
            variant: "Some".to_string(),
            fields: vec![value],
        }
    }

    /// `None`.
    pub fn none() -> Value {
        Value::Enum {
            name: QualName::builtin("Option"),
            variant: "None".to_string(),
            fields: Vec::new(),
        }
    }

    /// `Ok(v)`.
    pub fn ok(value: Value) -> Value {
        Value::Enum {
            name: QualName::builtin("Result"),
            variant: "Ok".to_string(),
            fields: vec![value],
        }
    }

    /// `Err(e)`.
    pub fn err(value: Value) -> Value {
        Value::Enum {
            name: QualName::builtin("Result"),
            variant: "Err".to_string(),
            fields: vec![value],
        }
    }

    /// `Some(v)` or `None`.
    pub fn option(value: Option<Value>) -> Value {
        match value {
            Some(value) => Value::some(value),
            None => Value::none(),
        }
    }

    /// The bytes of a `Bytes` value.
    pub fn bytes(bytes: &[u8]) -> Value {
        Value::List(bytes.iter().map(|b| Value::Byte(*b)).collect())
    }

    /// The size of the value in nodes (`docs/slice.md` section 15.3, O14):
    /// one for the value itself plus, recursively, its list items, map
    /// entries, record fields, tuple items and payloads. A `Text` counts one
    /// node per started 64 bytes, so that a text growing without bound
    /// also meets the limit.
    pub fn nodes(&self) -> u64 {
        match self {
            Value::Text(text) => 1 + text.len() as u64 / 64,
            Value::List(items) | Value::Tuple(items) => {
                1 + items.iter().map(Value::nodes).sum::<u64>()
            }
            Value::Record { fields, .. }
            | Value::Closure {
                captures: fields, ..
            } => 1 + fields.iter().map(|(_, v)| v.nodes()).sum::<u64>(),
            Value::Enum { fields, .. } => 1 + fields.iter().map(Value::nodes).sum::<u64>(),
            Value::Map(entries) => {
                1 + entries
                    .iter()
                    .map(|(k, v)| k.nodes() + v.nodes())
                    .sum::<u64>()
            }
            _ => 1,
        }
    }

    /// Whether the model can encode the value (`docs/slice.md` section
    /// 15.4): `Err` names the first part it cannot (a closure, an
    /// `IoError`), for E118.
    pub fn encodable(&self) -> Result<(), String> {
        match self {
            Value::Closure { .. } => Err("a closure".to_string()),
            Value::IoError { .. } => Err("an `IoError` value".to_string()),
            Value::List(items) | Value::Tuple(items) | Value::Enum { fields: items, .. } => {
                items.iter().try_for_each(Value::encodable)
            }
            Value::Record { fields, .. } => fields.iter().try_for_each(|(_, v)| v.encodable()),
            Value::Map(entries) => entries
                .iter()
                .try_for_each(|(k, v)| k.encodable().and_then(|()| v.encodable())),
            _ => Ok(()),
        }
    }

    /// The debug text of `docs/slice.md` section 13.6, quantities in the
    /// literal form of 14.1: what `debug()` prints on both targets, so the
    /// equivalence test (15.3) and `ignite explain` (15.8) share it. The
    /// one difference from the targets: a `Ref`, `Signal`, `SystemRef` or
    /// `Protocol` prints its name unquoted (15.8), where a compiled program
    /// prints it as `Text`.
    pub fn debug_text(&self) -> String {
        let mut out = String::new();
        self.write_debug(&mut out);
        out
    }

    fn write_debug(&self, out: &mut String) {
        match self {
            Value::Unit => out.push_str("()"),
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Value::Int(n) => out.push_str(&n.to_string()),
            Value::Byte(b) => out.push_str(&b.to_string()),
            Value::Text(text) => write_text(text, out),
            Value::List(items) => {
                out.push('[');
                write_items(items, out);
                out.push(']');
            }
            Value::Tuple(items) => {
                out.push('(');
                write_items(items, out);
                out.push(')');
            }
            Value::Record { name, fields } => {
                out.push_str(&name.name);
                if fields.is_empty() {
                    out.push_str(" {}");
                    return;
                }
                out.push_str(" { ");
                for (i, (field, value)) in fields.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    out.push_str(field);
                    out.push_str(": ");
                    value.write_debug(out);
                }
                out.push_str(" }");
            }
            Value::Enum {
                name,
                variant,
                fields,
            } => {
                let bare = name.is_builtin()
                    && matches!(name.name.as_str(), "Option" | "Result" | "Input");
                if !bare {
                    out.push_str(&name.name);
                    out.push('.');
                }
                out.push_str(variant);
                if !fields.is_empty() {
                    out.push('(');
                    write_items(fields, out);
                    out.push(')');
                }
            }
            Value::Map(entries) => {
                out.push('{');
                for (i, (key, value)) in entries.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    key.write_debug(out);
                    out.push_str(": ");
                    value.write_debug(out);
                }
                out.push('}');
            }
            Value::Quantity(kind, value) => out.push_str(&kind.debug_text(*value)),
            Value::Reference(reference) => out.push_str(&reference.text()),
            Value::Member { system, member } => {
                out.push_str(system);
                out.push('.');
                out.push_str(member);
            }
            Value::Module { module, .. } => {
                out.push_str("module(");
                write_text(module, out);
                out.push(')');
            }
            Value::Fn(_) | Value::Closure { .. } => out.push_str("<fn>"),
            Value::IoError { message, .. } => {
                out.push_str("IoError(");
                write_text(message, out);
                out.push(')');
            }
        }
    }
}

/// The value of type `ty` that `json` encodes; `path` locates `json` in
/// the error text (`value.fields.latency`).
fn decode(json: &Json, ty: &Ty, types: &TypedProgram, path: &str) -> Result<Value, DecodeError> {
    let fail = |message: String| DecodeError {
        message: format!("{path}: {message}"),
    };
    let expected = |what: &str| fail(format!("expected {what}, found {}", json.kind()));
    let tagged = |tag: &str| -> Result<&BTreeMap<String, Json>, DecodeError> {
        let object = json
            .as_object()
            .ok_or_else(|| expected(&format!("a `{tag}` object")))?;
        match object.get("type").and_then(Json::as_str) {
            Some(found) if found == tag => Ok(object),
            Some(found) => Err(fail(format!("expected type `{tag}`, found `{found}`"))),
            None => Err(fail(format!("expected type `{tag}`, found no `type` key"))),
        }
    };
    let text_of = |object: &BTreeMap<String, Json>, key: &str| -> Result<String, DecodeError> {
        object
            .get(key)
            .and_then(Json::as_str)
            .map(str::to_string)
            .ok_or_else(|| fail(format!("expected a `{key}` string")))
    };
    let items = |object: &BTreeMap<String, Json>, key: &str| -> Result<Vec<Json>, DecodeError> {
        match object.get(key) {
            Some(Json::Array(items)) => Ok(items.clone()),
            Some(other) => Err(fail(format!(
                "expected `{key}` to be an array, found {}",
                other.kind()
            ))),
            None => Ok(Vec::new()),
        }
    };
    let payload = |items: &[Json], tys: &[Ty], what: &str| -> Result<Vec<Value>, DecodeError> {
        if items.len() != tys.len() {
            return Err(fail(format!(
                "{what} takes {} values, found {}",
                tys.len(),
                items.len()
            )));
        }
        items
            .iter()
            .zip(tys)
            .enumerate()
            .map(|(i, (item, ty))| decode(item, ty, types, &format!("{path}[{i}]")))
            .collect()
    };
    let qualified = |text: &str| -> QualName {
        match text.rsplit_once('.') {
            Some((module, name)) => QualName::new(module, name),
            None => QualName::builtin(text),
        }
    };
    match ty {
        Ty::Unit => match json {
            Json::Null => Ok(Value::Unit),
            _ => Err(expected("null")),
        },
        Ty::Bool => json
            .as_bool()
            .map(Value::Bool)
            .ok_or_else(|| expected("a boolean")),
        Ty::Int => json
            .as_i64()
            .map(Value::Int)
            .ok_or_else(|| expected("an integer")),
        Ty::Text => json
            .as_str()
            .map(|t| Value::Text(t.to_string()))
            .ok_or_else(|| expected("a string")),
        Ty::Byte => {
            let object = tagged("Byte")?;
            let value = object.get("value").and_then(Json::as_i64);
            match value {
                Some(n) if (0..=255).contains(&n) => Ok(Value::Byte(n as u8)),
                _ => Err(fail("expected a `value` in 0..=255".to_string())),
            }
        }
        Ty::List(element) => {
            let items = json.as_array().ok_or_else(|| expected("an array"))?;
            items
                .iter()
                .enumerate()
                .map(|(i, item)| decode(item, element, types, &format!("{path}[{i}]")))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::List)
        }
        Ty::Option(inner) => {
            let object = tagged("Option")?;
            match text_of(object, "variant")?.as_str() {
                "Some" => Ok(Value::some(
                    payload(
                        &items(object, "fields")?,
                        std::slice::from_ref(inner),
                        "`Some`",
                    )?
                    .remove(0),
                )),
                "None" => Ok(Value::none()),
                other => Err(fail(format!("`Option` has no variant `{other}`"))),
            }
        }
        Ty::Result(ok, err) => {
            let object = tagged("Result")?;
            match text_of(object, "variant")?.as_str() {
                "Ok" => Ok(Value::ok(
                    payload(&items(object, "fields")?, std::slice::from_ref(ok), "`Ok`")?.remove(0),
                )),
                "Err" => Ok(Value::err(
                    payload(
                        &items(object, "fields")?,
                        std::slice::from_ref(err),
                        "`Err`",
                    )?
                    .remove(0),
                )),
                other => Err(fail(format!("`Result` has no variant `{other}`"))),
            }
        }
        Ty::Tuple(tys) => {
            let object = tagged("Tuple")?;
            payload(&items(object, "items")?, tys, "the tuple").map(Value::Tuple)
        }
        Ty::Record(q) | Ty::GenericRecord(q, _) => {
            let object = tagged(&q.to_string())?;
            let given = object
                .get("fields")
                .and_then(Json::as_object)
                .ok_or_else(|| fail("expected a `fields` object".to_string()))?;
            let declared = types
                .record_fields(q)
                .ok_or_else(|| fail(format!("unknown record `{q}`")))?;
            let args: &[Ty] = match ty {
                Ty::GenericRecord(_, args) => args,
                _ => &[],
            };
            let params = types.type_params(q).unwrap_or(&[]);
            let mut fields = Vec::with_capacity(declared.len());
            for (field, field_ty) in declared {
                let item = given
                    .get(field)
                    .ok_or_else(|| fail(format!("record `{q}` lacks the field `{field}`")))?;
                let field_ty = if args.is_empty() {
                    field_ty.clone()
                } else {
                    field_ty.substitute(params, args)
                };
                fields.push((
                    field.clone(),
                    decode(item, &field_ty, types, &format!("{path}.{field}"))?,
                ));
            }
            if let Some(extra) = given
                .keys()
                .find(|k| !declared.iter().any(|(f, _)| f == *k))
            {
                return Err(fail(format!("record `{q}` has no field `{extra}`")));
            }
            Ok(Value::Record {
                name: q.clone(),
                fields,
            })
        }
        Ty::Enum(q) | Ty::GenericEnum(q, _) => {
            let object = tagged(&q.to_string())?;
            let variant = text_of(object, "variant")?;
            let (_, tys) = types
                .variant(ty, &variant)
                .ok_or_else(|| fail(format!("enum `{q}` has no variant `{variant}`")))?;
            let fields = payload(&items(object, "fields")?, &tys, &format!("`{variant}`"))?;
            Ok(Value::Enum {
                name: q.clone(),
                variant,
                fields,
            })
        }
        Ty::Input => {
            let object = tagged("Input")?;
            let variant = text_of(object, "variant")?;
            let (_, tys) = types
                .variant(ty, &variant)
                .ok_or_else(|| fail(format!("`Input` has no variant `{variant}`")))?;
            let fields = payload(&items(object, "fields")?, &tys, &format!("`{variant}`"))?;
            Ok(Value::Enum {
                name: QualName::builtin("Input"),
                variant,
                fields,
            })
        }
        Ty::Map(key_ty, value_ty) => {
            let object = tagged("Map")?;
            let entries = items(object, "entries")?;
            entries
                .iter()
                .enumerate()
                .map(|(i, entry)| {
                    let pair = entry.as_array().filter(|p| p.len() == 2).ok_or_else(|| {
                        fail(format!("entry {i}: expected a `[key, value]` pair"))
                    })?;
                    Ok((
                        decode(&pair[0], key_ty, types, &format!("{path}[{i}].key"))?,
                        decode(&pair[1], value_ty, types, &format!("{path}[{i}].value"))?,
                    ))
                })
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Map)
        }
        Ty::Quantity(kind) => {
            let object = tagged(kind.name())?;
            let unit = match kind {
                QuantityKind::Duration => "ns",
                QuantityKind::ByteSize => "bytes",
                QuantityKind::Ratio => "ppm",
            };
            object
                .get(unit)
                .and_then(Json::as_i64)
                .map(|n| Value::Quantity(*kind, n))
                .ok_or_else(|| fail(format!("expected an integer `{unit}`")))
        }
        Ty::Ref(_) => {
            let text = text_of(tagged("Ref")?, "ref")?;
            let (system, member) = text
                .split_once('.')
                .ok_or_else(|| fail(format!("`{text}` is no `System.member` reference")))?;
            Ok(Value::Reference(Reference::Member {
                system: system.to_string(),
                member: member.to_string(),
            }))
        }
        Ty::Signal(_) => {
            let text = text_of(tagged("Signal")?, "ref")?;
            let parts: Vec<&str> = text.split('.').collect();
            if parts.len() != 3 {
                return Err(fail(format!("`{text}` is no `System.member.field` signal")));
            }
            Ok(Value::Reference(Reference::Signal {
                system: parts[0].to_string(),
                member: parts[1].to_string(),
                field: parts[2].to_string(),
            }))
        }
        Ty::SystemRef => Ok(Value::Reference(Reference::System(text_of(
            tagged("SystemRef")?,
            "ref",
        )?))),
        Ty::Protocol => Ok(Value::Reference(Reference::Protocol(qualified(&text_of(
            tagged("Protocol")?,
            "ref",
        )?)))),
        Ty::Fn(..) | Ty::EffectFn(..) => Ok(Value::Fn(qualified(&text_of(tagged("Fn")?, "fn")?))),
        Ty::TcpListener
        | Ty::TcpStream
        | Ty::PipeStream
        | Ty::IoError
        | Ty::Store(_)
        | Ty::Never
        | Ty::Param(_) => Err(fail(format!("a value of type `{ty}` cannot be decoded"))),
    }
}

fn write_items(items: &[Value], out: &mut String) {
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        item.write_debug(out);
    }
}

/// A `Text` in `"`, escaped as `docs/slice.md` section 13.6 says.
fn write_text(text: &str, out: &mut String) {
    out.push('"');
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
}
