use ignite_perf::json::{Json, obj, py_float};

use crate::common;

#[test]
fn floats_print_as_python_repr() {
    let cases = [
        (1.0, "1.0"),
        (0.8, "0.8"),
        (5.0, "5.0"),
        (-0.0, "-0.0"),
        (0.0, "0.0"),
        (7.19482421875, "7.19482421875"),
        (0.02897282800404355, "0.02897282800404355"),
        (0.0001, "0.0001"),
        (0.00001, "1e-05"),
        (1e16, "1e+16"),
        (1234567890123456.0, "1234567890123456.0"),
        (1.2345678901234568e17, "1.2345678901234568e+17"),
        (5e-324, "5e-324"),
        (f64::INFINITY, "Infinity"),
        (f64::NAN, "NaN"),
    ];
    for (value, text) in cases {
        assert_eq!(py_float(value), text, "{value:e}");
    }
}

#[test]
fn dumps_matches_python_separators_and_ascii_escapes() {
    let value = obj(vec![
        (
            "b",
            Json::Arr(vec![Json::Int(1), Json::Float(2.5), Json::Null]),
        ),
        ("a", "é\u{1f600}\"\\\n\u{1}\u{7f}".into()),
        ("c", obj::<&str>(vec![])),
        ("d", Json::Arr(vec![])),
    ]);
    assert_eq!(
        value.dumps(true),
        r#"{"a": "\u00e9\ud83d\ude00\"\\\n\u0001\u007f", "b": [1, 2.5, null], "c": {}, "d": []}"#
    );
    assert_eq!(
        value.dumps(false),
        r#"{"b": [1, 2.5, null], "a": "\u00e9\ud83d\ude00\"\\\n\u0001\u007f", "c": {}, "d": []}"#
    );
    assert_eq!(
        value.dumps_indent(true, 2),
        "{\n  \"a\": \"\\u00e9\\ud83d\\ude00\\\"\\\\\\n\\u0001\\u007f\",\n  \"b\": [\n    1,\n    2.5,\n    null\n  ],\n  \"c\": {},\n  \"d\": []\n}"
    );
    assert_eq!(Json::parse(&value.dumps(false)).unwrap(), value);
}

#[test]
fn equality_follows_python_dicts_and_numbers() {
    let a = Json::parse(r#"{"x": 5, "y": [1, 2]}"#).unwrap();
    let b = Json::parse(r#"{"y": [1.0, 2], "x": 5.0}"#).unwrap();
    assert_eq!(a, b);
    assert_ne!(a, Json::parse(r#"{"x": 5}"#).unwrap());
    let duplicate = Json::parse(r#"{"k": 1, "j": 2, "k": 3}"#).unwrap();
    assert_eq!(duplicate.dumps(false), r#"{"k": 3, "j": 2}"#);
}

#[test]
fn every_recorded_line_is_rewritten_byte_for_byte() {
    for line in common::read(&format!("{}/records.jsonl", common::RELEASE)).lines() {
        assert_eq!(Json::parse(line).unwrap().dumps(true), line);
    }
}

#[test]
fn indented_evidence_files_are_rewritten_byte_for_byte() {
    for name in [
        "run.json",
        "summary.json",
        "tools.json",
        "source-candidate.json",
    ] {
        let text = common::read(&format!("{}/{name}", common::RELEASE));
        assert_eq!(
            Json::parse(&text).unwrap().dumps_indent(true, 2) + "\n",
            text,
            "{name}"
        );
    }
    for name in ["report.json", "probe-timeout-command.json"] {
        let text = common::read(&format!("{}/{name}", common::READINESS));
        assert_eq!(
            Json::parse(&text).unwrap().dumps_indent(false, 2) + "\n",
            text,
            "{name}"
        );
    }
}
