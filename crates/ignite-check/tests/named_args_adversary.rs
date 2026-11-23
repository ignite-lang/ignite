//! Adversary (story `named-arguments-carry-parameter-effects`): call
//! shapes `named_args.rs` does not reach, each compared with the
//! positional call of the same callee (`docs/slice.md` 5.1 and 14.2). A
//! requirement `q.op` of a callee becomes `r.op` for each root `r` of the
//! argument passed for `q`, however that argument is passed.

use std::path::Path;

use crate::common;

use ignite_check::{Modules, check_module};
use ignite_syntax::{SourceFile, parse};

/// The code and message of each diagnostic of `text`, or the inferred
/// `requires` of `caller` when it checks clean.
fn outcome_with(text: &str, modules: &Modules) -> Result<Vec<String>, Vec<(String, String)>> {
    let source = SourceFile::new("t.ignite", text);
    let program = parse(&source).unwrap_or_else(|d| panic!("{}", source.render_all(&d)));
    match check_module(&program, modules) {
        Ok(symbols) => Ok(symbols.effects["caller"].requires.clone()),
        Err(diagnostics) => Err(diagnostics
            .iter()
            .map(|d| (d.code.to_string(), d.message.clone()))
            .collect()),
    }
}

fn outcome(text: &str) -> Result<Vec<String>, Vec<(String, String)>> {
    outcome_with(text, &Modules::new())
}

fn std_modules() -> Modules {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/std");
    common::load_std(&root).unwrap_or_else(|e| panic!("{e}")).1
}

const F: &str = "fn f(inp: TcpStream, out: TcpStream) -> Result<Unit, IoError> requires [out.write, inp.read] {\n  out.write_all(b\"x\")\n}\n";

/// Each call of `calls`, put in a caller without `requires` (`inferred`) and
/// in one whose `requires` is `[a.read]` (`declared`), gives the outcome
/// asserted, the same for every form.
fn same_for_all(prelude: &str, params: &str, calls: &[&str], inferred: &[&str], missing: &[&str]) {
    for call in calls {
        let open = format!(
            "package p\n\n{prelude}\nfn caller({params}) -> Result<Unit, IoError> {{\n  {call}\n}}\n"
        );
        assert_eq!(
            outcome(&open),
            Ok(inferred.iter().map(|s| s.to_string()).collect()),
            "inferred: {call}"
        );
        let declared = format!(
            "package p\n\n{prelude}\nfn caller({params}) -> Result<Unit, IoError> requires [a.read] {{\n  {call}\n}}\n"
        );
        let expected: Vec<(String, String)> = missing
            .iter()
            .map(|effect| {
                let (effect, what) = effect.split_once('@').unwrap();
                (
                    "E016".to_string(),
                    format!(
                        "missing effect `{effect}` for `{what}`; add it to the `requires` of `caller`"
                    ),
                )
            })
            .collect();
        assert_eq!(outcome(&declared), Err(expected), "declared: {call}");
    }
}

#[test]
fn adversary_a_ufcs_call_with_reordered_named_arguments_after_the_receiver() {
    let g = "fn g(r: TcpStream, inp: TcpStream, out: TcpStream) -> Result<Unit, IoError> requires [r.read, out.write, inp.read] {\n  out.write_all(b\"x\")\n}\n";
    same_for_all(
        g,
        "a: TcpStream, s: TcpStream, t: TcpStream",
        &[
            "t.g(a, s)",
            "t.g(inp = a, out = s)",
            "t.g(out = s, inp = a)",
            "g(out = s, r = t, inp = a)",
            "g(t, out = s, inp = a)",
        ],
        &["a.read", "s.write", "t.read"],
        &["t.read@g", "s.write@g"],
    );
}

#[test]
fn adversary_an_associated_fn_called_with_reordered_named_arguments() {
    let prelude = "type Box { n: Int }\n\nfn Box.f(inp: TcpStream, out: TcpStream) -> Result<Unit, IoError> requires [out.write, inp.read] {\n  out.write_all(b\"x\")\n}\n";
    same_for_all(
        prelude,
        "a: TcpStream, s: TcpStream",
        &[
            "Box.f(a, s)",
            "Box.f(out = s, inp = a)",
            "Box.f(inp = a, out = s)",
        ],
        &["a.read", "s.write"],
        &["s.write@Box.f"],
    );
}

#[test]
fn adversary_an_inferred_fn_reached_only_through_named_calls() {
    let prelude = format!(
        "{F}\nfn mid(x: TcpStream, y: TcpStream) -> Result<Unit, IoError> {{\n  f(out = x, inp = y)\n}}\n"
    );
    same_for_all(
        &prelude,
        "a: TcpStream, s: TcpStream",
        &["mid(s, a)", "mid(y = a, x = s)", "mid(x = s, y = a)"],
        &["a.read", "s.write"],
        &["s.write@mid"],
    );
}

#[test]
fn adversary_a_closure_capturing_a_stream_passed_by_name() {
    let missing = |what: &str| {
        (
            "E016".to_string(),
            format!("missing effect `s.write` for `{what}`; add it to the `requires` of `caller`"),
        )
    };
    for call in [
        "let k = || f(a, s)\n  k()",
        "let k = || f(out = s, inp = a)\n  k()",
        "let k = || f(inp = a, out = s)\n  k()",
    ] {
        let open = format!(
            "package p\n\n{F}\nfn caller(a: TcpStream, s: TcpStream) -> Result<Unit, IoError> {{\n  {call}\n}}\n"
        );
        assert_eq!(
            outcome(&open),
            Ok(vec!["a.read".to_string(), "s.write".to_string()]),
            "inferred: {call}"
        );
        let declared = format!(
            "package p\n\n{F}\nfn caller(a: TcpStream, s: TcpStream) -> Result<Unit, IoError> requires [a.read] {{\n  {call}\n}}\n"
        );
        assert_eq!(
            outcome(&declared),
            Err(vec![missing("closure"), missing("k")]),
            "declared: {call}"
        );
    }
}

#[test]
fn adversary_a_spawned_named_call_charges_like_the_positional_one() {
    let mut outcomes = Vec::new();
    for call in [
        "spawn f(a, s)",
        "spawn f(out = s, inp = a)",
        "spawn f(inp = a, out = s)",
    ] {
        let text = format!(
            "package p\n\n{F}\nfn caller(a: TcpStream, s: TcpStream) -> Result<Unit, IoError> {{\n  let t = {call}\n  Ok(Unit)\n}}\n"
        );
        outcomes.push((call, outcome(&text)));
    }
    let first = outcomes[0].1.clone();
    assert!(
        matches!(&first, Ok(r) if r.contains(&"s.write".to_string())),
        "positional spawn: {first:?}"
    );
    for (call, got) in &outcomes {
        assert_eq!(got, &first, "{call}");
    }
}

#[test]
fn adversary_an_imported_fn_called_with_reordered_named_arguments() {
    let modules = std_modules();
    for call in [
        "send(s, greet(\"x\"))",
        "send(greeting = greet(\"x\"), stream = s)",
        "send(stream = s, greeting = greet(\"x\"))",
    ] {
        let open = format!(
            "package p\n\nimport std.greeting.{{greet, send}}\n\nfn caller(a: TcpStream, s: TcpStream) -> Result<Unit, IoError> {{\n  {call}\n}}\n"
        );
        assert_eq!(
            outcome_with(&open, &modules),
            Ok(vec!["s.write".to_string()]),
            "{call}"
        );
        let declared = format!(
            "package p\n\nimport std.greeting.{{greet, send}}\n\nfn caller(a: TcpStream, s: TcpStream) -> Result<Unit, IoError> requires [a.write] {{\n  {call}\n}}\n"
        );
        assert_eq!(
            outcome_with(&declared, &modules),
            Err(vec![(
                "E016".to_string(),
                "missing effect `s.write` for `send`; add it to the `requires` of `caller`"
                    .to_string()
            )]),
            "{call}"
        );
    }
}
