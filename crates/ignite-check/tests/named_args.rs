//! Named arguments and parameter-scoped effects (`docs/slice.md` 5.1 and
//! 14.2): a requirement `q.op` of a callee becomes `r.op` for each root `r`
//! of the argument passed for `q`, whether that argument is passed by
//! position or by name, in declared order or not. The maintainer review of
//! 2026-11-23 found the named forms charged nothing, or the argument in
//! that source position.

use ignite_check::{Modules, check_module};
use ignite_syntax::{SourceFile, parse};

/// A callee with two stream parameters whose `requires` names the second.
const CALLEE: &str = "fn f(inp: TcpStream, out: TcpStream) -> Result<Unit, IoError> requires [out.write] {\n  out.write_all(b\"x\")\n}\n";

/// The three ways to pass `a` for `inp` and `s` for `out`.
const CALLS: [&str; 3] = ["f(a, s)", "f(inp = a, out = s)", "f(out = s, inp = a)"];

/// The code and message of each diagnostic of `text`, or the inferred
/// `requires` of `caller` when it checks clean.
fn outcome(text: &str) -> Result<Vec<String>, Vec<(String, String)>> {
    let source = SourceFile::new("t.ignite", text);
    let program = parse(&source).unwrap_or_else(|d| panic!("{}", source.render_all(&d)));
    match check_module(&program, &Modules::new()) {
        Ok(symbols) => Ok(symbols.effects["caller"].requires.clone()),
        Err(diagnostics) => Err(diagnostics
            .iter()
            .map(|d| (d.code.to_string(), d.message.clone()))
            .collect()),
    }
}

fn program(callee: &str, caller: &str) -> String {
    format!("package p\n\n{callee}\n{caller}")
}

#[test]
fn a_named_call_misses_the_same_effect_as_the_positional_call() {
    for call in CALLS {
        let caller = format!(
            "fn caller(a: TcpStream, s: TcpStream) -> Result<Unit, IoError> requires [a.read] {{\n  {call}\n}}\n"
        );
        assert_eq!(
            outcome(&program(CALLEE, &caller)),
            Err(vec![(
                "E016".to_string(),
                "missing effect `s.write` for `f`; add it to the `requires` of `caller`"
                    .to_string()
            )]),
            "{call}"
        );
    }
}

#[test]
fn a_named_call_infers_the_same_effect_as_the_positional_call() {
    for call in CALLS {
        let caller = format!(
            "fn caller(a: TcpStream, s: TcpStream) -> Result<Unit, IoError> {{\n  {call}\n}}\n"
        );
        assert_eq!(
            outcome(&program(CALLEE, &caller)),
            Ok(vec!["s.write".to_string()]),
            "{call}"
        );
    }
}

#[test]
fn an_ambient_requirement_is_the_same_for_every_form() {
    let callee = "fn f(inp: TcpStream, out: TcpStream) -> Result<Unit, IoError> requires [stdout.write] {\n  Stdout.write_all(b\"x\")\n}\n";
    for call in CALLS {
        let caller = format!(
            "fn caller(a: TcpStream, s: TcpStream) -> Result<Unit, IoError> {{\n  {call}\n}}\n"
        );
        assert_eq!(
            outcome(&program(callee, &caller)),
            Ok(vec!["stdout.write".to_string()]),
            "{call}"
        );
    }
}

#[test]
fn a_method_call_keeps_its_receiver_first_and_resolves_named_arguments() {
    let callee = "fn g(out: TcpStream, inp: TcpStream) -> Result<Unit, IoError> requires [out.write, inp.read] {\n  out.write_all(b\"x\")\n}\n";
    for call in ["s.g(a)", "s.g(inp = a)", "g(s, a)", "g(inp = a, out = s)"] {
        let caller = format!(
            "fn caller(a: TcpStream, s: TcpStream) -> Result<Unit, IoError> {{\n  {call}\n}}\n"
        );
        assert_eq!(
            outcome(&program(callee, &caller)),
            Ok(vec!["a.read".to_string(), "s.write".to_string()]),
            "{call}"
        );
    }
}

#[test]
fn a_fn_value_call_charges_the_named_argument_like_the_positional_one() {
    for call in ["h(s)", "h(x = s)"] {
        let caller = format!(
            "fn caller(h: fn(x: TcpStream) -> Int requires [x.write], s: TcpStream) -> Int {{\n  return {call}\n}}\n"
        );
        assert_eq!(outcome(&caller), Ok(vec!["s.write".to_string()]), "{call}");
    }
}

#[test]
fn a_call_result_keeps_the_roots_of_a_named_argument() {
    let callee = "fn pass(n: Int, x: TcpStream) -> TcpStream {\n  return x\n}\n";
    for call in ["pass(1, s)", "pass(n = 1, x = s)", "pass(x = s, n = 1)"] {
        let caller = format!(
            "fn caller(s: TcpStream) -> Result<Unit, IoError> {{\n  let t = {call}\n  t.write_all(b\"x\")\n}}\n"
        );
        assert_eq!(
            outcome(&program(callee, &caller)),
            Ok(vec!["s.write".to_string()]),
            "{call}"
        );
    }
}
