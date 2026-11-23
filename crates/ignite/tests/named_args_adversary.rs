//! Adversary (story `named-arguments-carry-parameter-effects`): named
//! arguments through the whole pipeline (`docs/slice.md` 5.1 and 14.2).
//! `ignite check --json` reports a named call like the positional one, a
//! named call of a fn value stays refused, and the evaluator and both
//! targets bind each named argument to the parameter it names while
//! evaluating the arguments in source order.

use std::path::{Path, PathBuf};
use std::process::Command;

use ignite_eval::Json;

use crate::borrowed_reads_adversary::agree;

fn program_file(name: &str, text: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("named-args-adversary")
        .join(name);
    std::fs::create_dir_all(&dir).expect("the source directory");
    let file = dir.join("program.ignite");
    std::fs::write(&file, text).expect("write the program");
    file
}

/// `(code, message)` of each diagnostic `ignite check --json` reports.
fn check_json(name: &str, text: &str) -> Vec<(String, String)> {
    let file = program_file(name, text);
    let output = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .args(["check", "--json"])
        .arg(&file)
        .output()
        .expect("run ignite check");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let document = Json::parse(stdout.trim_end()).unwrap_or_else(|e| panic!("{e}: {stdout}"));
    let Some(Json::Array(items)) = document.get("diagnostics") else {
        panic!("no diagnostics: {stdout}");
    };
    let field = |item: &Json, key: &str| match item.get(key) {
        Some(Json::Str(text)) => text.clone(),
        other => panic!("{key}: {other:?}"),
    };
    items
        .iter()
        .map(|item| (field(item, "code"), field(item, "message")))
        .collect()
}

const F: &str = "fn f(inp: TcpStream, out: TcpStream) -> Result<Unit, IoError> requires [out.write, inp.read] {\n  out.write_all(b\"x\")\n}\n";

#[test]
fn adversary_check_json_reports_a_reordered_named_call_like_the_positional_one() {
    let expected = vec![(
        "E016".to_string(),
        "missing effect `s.write` for `f`; add it to the `requires` of `caller`".to_string(),
    )];
    for (index, call) in ["f(a, s)", "f(out = s, inp = a)", "f(a, out = s)"]
        .iter()
        .enumerate()
    {
        let text = format!(
            "package p\n\n{F}\nfn caller(a: TcpStream, s: TcpStream) -> Result<Unit, IoError> requires [a.read] {{\n  {call}\n}}\n"
        );
        assert_eq!(
            check_json(&format!("e016-{index}"), &text),
            expected,
            "{call}"
        );
    }
}

#[test]
fn adversary_a_named_call_of_a_fn_value_with_a_parameter_requirement_is_refused() {
    let text = "package p\n\nfn caller(h: fn(x: TcpStream) -> Int requires [x.write], s: TcpStream) -> Int requires [s.write] {\n  return h(x = s)\n}\n";
    let codes: Vec<String> = check_json("fn-value", text)
        .into_iter()
        .map(|(code, _)| code)
        .collect();
    assert_eq!(codes, ["E103"]);
}

/// Each named call evaluates its arguments in source order (`log` records
/// the order, one digit per argument) and binds each to the parameter it
/// names, for a direct call, an associated fn, a UFCS call after its
/// receiver and a generic fn.
const ORDER: &str = r#"package named_order

type Box {
  n: Int
}

fn sub(a: Int, b: Int) -> Int = a * 100 - b

fn Box.sub3(a: Int, b: Int, c: Int) -> Int = a * 10000 + b * 100 + c

fn weigh(x: Box, a: Int, b: Int) -> Int = x.n * 10000 + a * 100 + b

fn pick<T>(first: T, second: T) -> T = first

fn direct() -> Int {
  var log = 0
  let r = sub(b = if true {
    log = log * 10 + 1
    3
  } else {
    0
  }, a = if true {
    log = log * 10 + 2
    5
  } else {
    0
  })
  r * 1000 + log
}

fn associated() -> Int {
  var log = 0
  let r = Box.sub3(c = if true {
    log = log * 10 + 1
    1
  } else {
    0
  }, a = if true {
    log = log * 10 + 2
    2
  } else {
    0
  }, b = if true {
    log = log * 10 + 3
    3
  } else {
    0
  })
  r * 1000 + log
}

fn receiver() -> Int {
  var log = 0
  let bx = Box { n = 9 }
  let r = bx.weigh(b = if true {
    log = log * 10 + 1
    4
  } else {
    0
  }, a = if true {
    log = log * 10 + 2
    7
  } else {
    0
  })
  r * 1000 + log
}

fn generic() -> Int {
  var log = 0
  let r = pick(second = if true {
    log = log * 10 + 1
    "y"
  } else {
    ""
  }, first = if true {
    log = log * 10 + 2
    "x"
  } else {
    ""
  })
  if r == "x" {
    log
  } else {
    0 - log
  }
}

test "a direct named call binds by name in source order" {
  assert direct() == 497012
}

test "an associated named call binds by name in source order" {
  assert associated() == 20301123
}

test "a ufcs named call binds by name after its receiver" {
  assert receiver() == 90704012
}

test "a generic named call binds by name in source order" {
  assert generic() == 12
}
"#;

#[test]
fn adversary_named_arguments_bind_alike_on_the_evaluator_and_both_targets() {
    agree("named_order", ORDER, 4);
}
