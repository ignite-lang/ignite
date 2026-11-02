//! Values of the evaluator (`docs/slice.md` sections 15.3 and 15.4) and
//! their debug text (section 13.6, 14.1 quantities in literal form). The
//! model encodes a [`Value`] as the JSON of the 15.4 table. Owner: P2.

use ignite_types::{QualName, QuantityKind, Reference};

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
