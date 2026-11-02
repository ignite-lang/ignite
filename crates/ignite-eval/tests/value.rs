//! `Value` (`docs/slice.md` sections 15.3 and 15.4): debug text of
//! section 13.6, node counts of O14, what the model can encode, and
//! `Json::parse` as the inverse of `Json::compact` (15.4, O17).

use ignite_eval::{Json, Value};
use ignite_types::{QualName, QuantityKind, Reference};

fn record(name: &str, fields: Vec<(&str, Value)>) -> Value {
    Value::Record {
        name: QualName::new("m", name),
        fields: fields
            .into_iter()
            .map(|(f, v)| (f.to_string(), v))
            .collect(),
    }
}

fn variant(enum_: QualName, variant: &str, fields: Vec<Value>) -> Value {
    Value::Enum {
        name: enum_,
        variant: variant.to_string(),
        fields,
    }
}

fn text(s: &str) -> Value {
    Value::Text(s.to_string())
}

#[test]
fn scalars_and_text_escapes() {
    assert_eq!(Value::Unit.debug_text(), "()");
    assert_eq!(Value::Bool(true).debug_text(), "true");
    assert_eq!(Value::Int(-42).debug_text(), "-42");
    assert_eq!(Value::Byte(255).debug_text(), "255");
    assert_eq!(
        text("q\" bs\\ nl\n cr\r tab\t nul\0 del\u{7f} bell\u{7} é").debug_text(),
        r#""q\" bs\\ nl\n cr\r tab\t nul\0 del\u{7f} bell\u{7} é""#
    );
}

#[test]
fn collections() {
    let list = Value::List(vec![Value::Int(1), Value::Int(2)]);
    assert_eq!(list.debug_text(), "[1, 2]");
    assert_eq!(Value::List(Vec::new()).debug_text(), "[]");
    assert_eq!(Value::bytes(b"hi").debug_text(), "[104, 105]");
    let tuple = Value::Tuple(vec![Value::Int(1), text("a"), Value::Unit]);
    assert_eq!(tuple.debug_text(), r#"(1, "a", ())"#);
    let map = Value::Map(vec![(text("a"), Value::Int(1)), (text("b"), Value::Int(2))]);
    assert_eq!(map.debug_text(), r#"{"a": 1, "b": 2}"#);
    assert_eq!(Value::Map(Vec::new()).debug_text(), "{}");
}

#[test]
fn records_and_enums() {
    let todo = record(
        "Todo",
        vec![
            ("id", Value::Int(1)),
            ("title", text("milk")),
            ("done", Value::Bool(false)),
        ],
    );
    assert_eq!(
        todo.debug_text(),
        r#"Todo { id: 1, title: "milk", done: false }"#
    );
    assert_eq!(record("Empty", Vec::new()).debug_text(), "Empty {}");
    assert_eq!(Value::some(Value::Int(1)).debug_text(), "Some(1)");
    assert_eq!(Value::none().debug_text(), "None");
    assert_eq!(
        Value::ok(Value::bytes(b"hi")).debug_text(),
        "Ok([104, 105])"
    );
    assert_eq!(Value::err(text("no")).debug_text(), r#"Err("no")"#);
    assert_eq!(
        variant(QualName::builtin("Input"), "Data", vec![Value::bytes(b"a")]).debug_text(),
        "Data([97])"
    );
    assert_eq!(
        variant(QualName::builtin("Exposure"), "Public", Vec::new()).debug_text(),
        "Exposure.Public"
    );
    let shape = QualName::new("m", "Shape");
    assert_eq!(
        variant(shape.clone(), "Dot", Vec::new()).debug_text(),
        "Shape.Dot"
    );
    assert_eq!(
        variant(shape, "Rect", vec![Value::Int(2), Value::Int(4)]).debug_text(),
        "Shape.Rect(2, 4)"
    );
}

#[test]
fn quantities_print_the_largest_exact_unit() {
    assert_eq!(
        Value::Quantity(QuantityKind::Duration, 1_500_000_000).debug_text(),
        "1500 ms"
    );
    assert_eq!(
        Value::Quantity(QuantityKind::Duration, 120_000_000_000).debug_text(),
        "2 minutes"
    );
    assert_eq!(
        Value::Quantity(QuantityKind::ByteSize, 1 << 20).debug_text(),
        "1 MiB"
    );
    assert_eq!(
        Value::Quantity(QuantityKind::ByteSize, 1000).debug_text(),
        "1000 bytes"
    );
    assert_eq!(
        Value::Quantity(QuantityKind::Ratio, 999_000).debug_text(),
        "99.9%"
    );
}

#[test]
fn references_fns_modules_and_errors() {
    let member = Reference::Member {
        system: "Terminal".to_string(),
        member: "ingress".to_string(),
    };
    assert_eq!(Value::Reference(member).debug_text(), "Terminal.ingress");
    assert_eq!(
        Value::Member {
            system: "T".to_string(),
            member: "m".to_string()
        }
        .debug_text(),
        "T.m"
    );
    assert_eq!(Value::Fn(QualName::new("m", "f")).debug_text(), "<fn>");
    let closure = Value::Closure {
        module: "m".to_string(),
        offset: 7,
        captures: vec![("n".to_string(), Value::Int(1))],
    };
    assert_eq!(closure.debug_text(), "<fn>");
    assert_eq!(
        Value::Module {
            module: "examples.terminal.commands".to_string(),
            fns: Vec::new()
        }
        .debug_text(),
        r#"module("examples.terminal.commands")"#
    );
    assert_eq!(
        Value::IoError {
            message: "boom".to_string(),
            exit_code: 1
        }
        .debug_text(),
        r#"IoError("boom")"#
    );
}

#[test]
fn nodes_count_items_recursively() {
    assert_eq!(Value::Int(1).nodes(), 1);
    assert_eq!(text("short").nodes(), 1);
    assert_eq!(text(&"x".repeat(200)).nodes(), 4);
    let list = Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
    assert_eq!(list.nodes(), 4);
    let map = Value::Map(vec![(text("a"), list.clone())]);
    assert_eq!(map.nodes(), 6);
    assert_eq!(Value::some(list).nodes(), 5);
    assert_eq!(
        record("R", vec![("a", Value::Int(1)), ("b", Value::Int(2))]).nodes(),
        3
    );
}

#[test]
fn closures_and_io_errors_are_unencodable() {
    let closure = Value::Closure {
        module: "m".to_string(),
        offset: 0,
        captures: Vec::new(),
    };
    assert_eq!(closure.encodable(), Err("a closure".to_string()));
    let nested = record("R", vec![("f", Value::some(closure))]);
    assert_eq!(nested.encodable(), Err("a closure".to_string()));
    let error = Value::IoError {
        message: "x".to_string(),
        exit_code: 2,
    };
    assert_eq!(
        Value::List(vec![error]).encodable(),
        Err("an `IoError` value".to_string())
    );
    assert_eq!(Value::Fn(QualName::new("m", "f")).encodable(), Ok(()));
    assert_eq!(
        Value::Map(vec![(text("k"), Value::some(Value::Int(1)))]).encodable(),
        Ok(())
    );
}

#[test]
fn json_parse_is_the_inverse_of_compact() {
    let texts = [
        r#"{"a":[1,-2,true,false,null,"x\"y\\z\n\u0001"],"b":{"c":"é😀"}}"#,
        "[]",
        "{}",
        r#""plain""#,
        "0",
        "-9223372036854775808",
    ];
    for text in texts {
        let json = Json::parse(text).unwrap_or_else(|e| panic!("{text}: {e}"));
        assert_eq!(Json::parse(&json.compact()).unwrap(), json, "{text}");
    }
    assert_eq!(
        Json::parse(" { \"k\" : [ 1 , 2 ] } ").unwrap().compact(),
        r#"{"k":[1,2]}"#
    );
    let quoted = concat!(
        r#""\u0000\u0001\u0002\u0003\u0004\u0005\u0006\u0007\b\t\n\u000b\f\r"#,
        r#"\u000e\u000f\u0010\u0011\u0012\u0013\u0014\u0015\u0016\u0017"#,
        r#"\u0018\u0019\u001a\u001b\u001c\u001d\u001e\u001f\"\\é😀"#,
        "\u{7f}\u{2028}\u{2029}\""
    );
    let text = Json::parse(quoted).unwrap().as_str().unwrap().to_string();
    let object = Json::Object(std::collections::BTreeMap::from([(
        text.clone(),
        Json::Str(text),
    )]));
    assert_eq!(object.compact(), format!("{{{quoted}:{quoted}}}"));
    for indent in [0, 3, 20, 100] {
        let mut pretty = "existing\n".to_string();
        object.write(&mut pretty, indent);
        assert_eq!(
            pretty,
            format!(
                "existing\n{{\n{}{quoted}: {quoted}\n{}}}",
                "  ".repeat(indent + 1),
                "  ".repeat(indent)
            )
        );
        assert_eq!(Json::parse(&pretty["existing\n".len()..]).unwrap(), object);
    }
    for bad in [
        "",
        "{",
        "[1,]",
        r#"{"a":1,"a":2}"#,
        "1.5",
        "1e3",
        "tru",
        r#""\x""#,
        "[1] x",
    ] {
        assert!(Json::parse(bad).is_err(), "{bad:?} parsed");
    }
}
