//! `Value::decode`, the inverse of the 15.4 encoding (`docs/slice.md`
//! 16.16): every `std.control` value the driver journals round-trips
//! through `value_json` and `decode` against the typed program of
//! `std.control`, and a wrong tag, a missing field or an unknown variant
//! is a `DecodeError` naming its path. `Json::parse` on its own is tested
//! in `ignite-eval`'s `tests/value.rs`.

use std::path::{Path, PathBuf};

use ignite_eval::{CONTROL_MODULE, Evaluator, Fuel, Value, value_json};
use ignite_types::{QualName, Ty};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/control_trace.ignite")
}

fn design() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/terminal/terminal.design.ignite")
}

fn control(name: &str) -> Ty {
    Ty::Record(QualName::new(CONTROL_MODULE, name))
}

fn control_enum(name: &str) -> Ty {
    Ty::Enum(QualName::new(CONTROL_MODULE, name))
}

/// `value` encoded, parsed and decoded as `ty` is `value` again.
fn round_trip(loaded: &ignite::Loaded, value: &Value, ty: &Ty) {
    let modules = loaded.modules();
    let types = modules.get(CONTROL_MODULE).expect("std.control").types;
    let text = value_json(value).compact();
    let decoded = Value::decode(&text, ty, types).unwrap_or_else(|e| panic!("{text}: {e}"));
    assert_eq!(&decoded, value, "{text}");
    let pretty = {
        let mut out = String::new();
        value_json(value).write(&mut out, 0);
        out
    };
    let decoded = Value::decode(&pretty, ty, types).unwrap_or_else(|e| panic!("{pretty}: {e}"));
    assert_eq!(&decoded, value);
}

#[test]
fn machines_of_the_design_model_round_trip() {
    let loaded = ignite::load(&design(), None).unwrap_or_else(|e| panic!("{e}"));
    let model = loaded
        .model(&ignite::PlanOptions::default())
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(model.machines.len() >= 6, "{}", model.machines.len());
    for machine in &model.machines {
        round_trip(&loaded, &machine.machine, &control_enum("Machine"));
    }
}

#[test]
fn events_effects_and_standings_of_the_trace_round_trip() {
    let loaded = ignite::load(&fixture(), None).unwrap_or_else(|e| panic!("{e}"));
    let modules = loaded.modules();
    let mut evaluator = Evaluator::new(&modules, Fuel::default());
    let module = loaded.types().module.clone();
    let call = |evaluator: &mut Evaluator<'_>, name: &str| {
        evaluator
            .call(&QualName::new(module.clone(), name), Vec::new())
            .unwrap_or_else(|e| panic!("{name}: {e:?}"))
    };
    let mut values = Vec::new();
    for (name, ty) in [
        ("supervisor_events", control("Event")),
        ("scheduler_events", control("Event")),
        ("rolling_events", control("Event")),
        ("calendar_events", control("Event")),
    ] {
        let Value::List(items) = call(&mut evaluator, name) else {
            panic!("{name} is no list");
        };
        assert!(!items.is_empty(), "{name}");
        values.extend(items.into_iter().map(|item| (item, ty.clone())));
    }
    for name in [
        "supervisor_effects",
        "stop_effects",
        "scheduler_effects",
        "rolling_effects",
        "calendar_effects",
    ] {
        let Value::List(items) = call(&mut evaluator, name) else {
            panic!("{name} is no list");
        };
        for item in items {
            if let Value::Enum {
                variant, fields, ..
            } = &item
            {
                match variant.as_str() {
                    "Publish" => values.push((fields[0].clone(), control("Standing"))),
                    "Fire" => values.push((fields[0].clone(), control("Fired"))),
                    _ => {}
                }
            }
            values.push((item, control_enum("Effect")));
        }
    }
    for name in [
        "supervisor_machine",
        "scheduler_machine",
        "rolling_machine",
        "calendar_machine",
        "a7_machine",
    ] {
        values.push((call(&mut evaluator, name), control_enum("Machine")));
    }
    assert!(values.len() > 40, "{}", values.len());
    for (value, ty) in &values {
        round_trip(&loaded, value, ty);
    }
}

#[test]
fn decode_errors_name_the_path() {
    let loaded = ignite::load(&fixture(), None).unwrap_or_else(|e| panic!("{e}"));
    let modules = loaded.modules();
    let types = modules.get(CONTROL_MODULE).expect("std.control").types;
    let event = control("Event");
    let cases: [(&str, &str); 5] = [
        (
            r#"{"fields":{"at":{"ns":0,"type":"Duration"},"kind":{"fields":[],"type":"std.control.EventKind","variant":"Released"}},"type":"std.control.Effect"}"#,
            "expected type `std.control.Event`, found `std.control.Effect`",
        ),
        (
            r#"{"fields":{"kind":{"fields":[],"type":"std.control.EventKind","variant":"Released"}},"type":"std.control.Event"}"#,
            "at",
        ),
        (
            r#"{"fields":{"at":{"ns":0,"type":"Duration"},"kind":{"fields":[],"type":"std.control.EventKind","variant":"Vanished"}},"type":"std.control.Event"}"#,
            "Vanished",
        ),
        (
            r#"{"fields":{"at":{"ns":"zero","type":"Duration"},"kind":{"fields":[],"type":"std.control.EventKind","variant":"Released"}},"type":"std.control.Event"}"#,
            "value.at",
        ),
        ("{\"fields\":", "expected a value"),
    ];
    for (text, expected) in cases {
        let error = Value::decode(text, &event, types).expect_err(text);
        assert!(
            error.message.contains(expected),
            "{text}\n  got: {}\n  expected a mention of: {expected}",
            error.message
        );
    }
}
