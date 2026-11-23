//! Adversary cases for story
//! `llvm-short-circuit-with-returning-operand-verifies` (IG-07): an operand
//! that leaves its block (`return`, `break`, `continue`, a `match` or `if`
//! whose every arm leaves) in the positions the unit's fixture
//! (`fixtures/tests/short_circuit_return.ignite`) does not reach. Each
//! program is run by the evaluator (`Evaluator::run_test`) and by
//! `ignite test` on both targets; a program that checks and does not build
//! on a target fails its test.
//!
//! - [`COMPARED`]: a left operand of `==`, `!=`, `<` of type `Never`, on
//!   `Bool`, `Text`, `Byte` and `U32`. The Rust target writes
//!   `(return a) == p`, which rustc refuses (`! == bool`); before the
//!   unit it wrote `return a == p`, which returned `a == p`.
//! - [`ARGUMENTS`]: a `Never` argument of a parameter the Rust target
//!   lends (`f(&(return a))`), and the `Never` left operand of a record
//!   `==` and of a list `+`.
//! - [`LOOP_CONDITION`]: `break` and `continue` in the condition of a
//!   `while` nested in another loop. The typer marks the enclosing loop
//!   (the condition is typed before the `while`'s loop is pushed); the
//!   evaluator and the LLVM target leave or continue that loop; rustc
//!   refuses an unlabeled `break` in a `while` condition (E0590).
//! - [`OUTSIDE_LOOP`]: `break` as a short-circuit operand with no loop
//!   around it, and in the condition of a `while` that is not inside
//!   another loop. The program checks; the evaluator has no loop to leave;
//!   the LLVM lowering panics (`inside a loop`) and rustc refuses it. It
//!   must be refused by the checker or build on both targets.
//! - [`GUARDS`]: positions that agree today and no other test holds:
//!   chains of three with a returning operand in each place, `?` inside an
//!   operand, a right operand whose every `if` or `match` arm returns,
//!   returns inside closures and test bodies, and a `Never` operand of
//!   arithmetic, an index, a tuple, a record, an `if` condition, a `match`
//!   scrutinee and a `let`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use ignite_eval::{Evaluator, Fuel};
use ignite_syntax::ast::Decl;

use crate::common;

/// One Rust build at a time: the programs share a cargo target directory,
/// so `ignite-std` is built once.
static RUST: Mutex<()> = Mutex::new(());

fn root() -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join("short-circuit-adversary")
}

/// `program` written to `<tmp>/short-circuit-adversary/<name>/<name>.ignite`.
fn source(name: &str, program: &str) -> PathBuf {
    let dir = root().join(name);
    std::fs::create_dir_all(&dir).expect("the source directory");
    let file = dir.join(format!("{name}.ignite"));
    std::fs::write(&file, program).expect("write the program");
    file
}

/// What went wrong with the inline tests of `file` in the evaluator and on
/// both targets; empty when every test passes on all three.
fn failures(name: &str, file: &Path) -> Vec<String> {
    let mut failed = Vec::new();
    let loaded = ignite::load(file, None).unwrap_or_else(|e| panic!("{name} checks: {e}"));
    let modules = loaded.modules();
    let mut evaluator = Evaluator::new(&modules, Fuel::default());
    let module = loaded.types().module.clone();
    for decl in &loaded.program().decls {
        if let Decl::Test(test) = decl
            && let Err(error) = evaluator.run_test(&module, &test.name.name)
        {
            failed.push(format!("evaluator: {}: {error:?}", test.name.name));
        }
    }
    for target in ["rust", "llvm"] {
        let _guard = (target == "rust").then(|| RUST.lock().unwrap_or_else(|e| e.into_inner()));
        let mut command = Command::new(env!("CARGO_BIN_EXE_ignite"));
        command
            .arg("test")
            .arg(file)
            .args(["--target", target, "--jobs", "2", "--out"])
            .arg(root().join(name).join(target))
            .arg("--runtime")
            .arg(common::runner_archive());
        if target == "rust" {
            command.arg("--target-dir").arg(root().join("cargo"));
        }
        let output = command.output().expect("run ignite test");
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let errors: Vec<&str> = stderr
                .lines()
                .filter(|line| {
                    line.starts_with("error")
                        || line.contains("panicked")
                        || line.contains("return")
                        || line.contains("break")
                        || line.contains("continue")
                })
                .collect();
            failed.push(format!(
                "{target}: exit {:?}\n{}\n{}",
                output.status.code(),
                String::from_utf8_lossy(&output.stdout).trim(),
                errors.join("\n")
            ));
        }
    }
    failed
}

fn assert_agree(name: &str, program: &str) {
    let file = source(name, program);
    let failed = failures(name, &file);
    assert!(failed.is_empty(), "{name}:\n{}", failed.join("\n\n"));
}

// ---- a `Never` left operand of a comparison --------------------------------

const COMPARED: &str = r#"package sc_adversary.compared

fn eq_left(a: Bool, p: Bool) -> Bool {
  (return a) == p
}

fn ne_left(a: Bool, p: Bool) -> Bool {
  (return a) != p
}

fn lt_left(a: Bool, x: Int) -> Bool {
  (return a) < x
}

fn text_literal(a: Bool) -> Bool {
  (return a) == "x"
}

fn text_lt(a: Bool, t: Text) -> Bool {
  (return a) < t
}

fn byte_eq(a: Bool, b: Byte) -> Bool {
  (return a) == b
}

fn u32_lt(a: Bool, b: U32) -> Bool {
  (return a) < b
}

fn in_right_operand(p: Bool, q: Bool) -> Bool {
  p && ((return q) == true)
}

fn every_arm_returns(o: Option<Int>, x: Int) -> Bool {
  (match o { Some(_) => return true, None => return false }) == x
}

test "a left operand that returns is the value of a comparison" {
  assert eq_left(true, false) == true
  assert eq_left(false, false) == false
  assert ne_left(true, true) == true
  assert lt_left(true, 1) == true
  assert text_literal(true) == true
  assert text_lt(true, "a") == true
  assert byte_eq(true, b'a') == true
  assert u32_lt(true, 1.wrap_u32()) == true
  assert in_right_operand(true, false) == false
  assert in_right_operand(false, true) == false
  assert every_arm_returns(Some(1), 1) == true
  assert every_arm_returns(None, 0) == false
}
"#;

#[test]
fn a_never_left_operand_of_a_comparison_builds_and_returns() {
    assert_agree("compared", COMPARED);
}

// ---- a `Never` argument the Rust target lends --------------------------------

const ARGUMENTS: &str = r#"package sc_adversary.arguments

type Point { x: Int, y: Int }

fn add2(a: Int, b: Int) -> Int = a + b

fn length(t: Text) -> Int = t.len()

fn int_argument(a: Int, x: Int) -> Int {
  add2(return a, x)
}

fn text_argument(a: Int) -> Int {
  length(return a)
}

fn record_eq(a: Bool, p: Point) -> Bool {
  (return a) == p
}

fn list_concat(a: List<Int>, b: List<Int>) -> List<Int> {
  (return a) + b
}

test "a returning argument or aggregate operand is the value" {
  assert int_argument(1, 2) == 1
  assert text_argument(5) == 5
  assert record_eq(true, Point { x = 1, y = 2 }) == true
  assert list_concat([1], [2]) == [1]
}
"#;

#[test]
fn a_never_argument_or_aggregate_operand_builds_and_returns() {
    assert_agree("arguments", ARGUMENTS);
}

// ---- `break` and `continue` in the condition of a nested `while` ------------

const LOOP_CONDITION: &str = r#"package sc_adversary.loop_condition

fn inner_break(p: Bool) -> Int {
  var n = 0
  var j = 0
  while j < 3 {
    j = j + 1
    var i = 0
    while i < 5 && (p || (break)) {
      i = i + 1
      n = n + 1
    }
    n = n + 10
  }
  n
}

fn inner_continue(p: Bool) -> Int {
  var n = 0
  var j = 0
  while j < 3 {
    j = j + 1
    var i = 0
    while i < 2 && (p || (continue)) {
      i = i + 1
      n = n + 1
    }
    n = n + 10
  }
  n
}

test "break in a while condition leaves the enclosing loop" {
  assert inner_break(true) == 45
  assert inner_break(false) == 0
}

test "continue in a while condition continues the enclosing loop" {
  assert inner_continue(true) == 36
  assert inner_continue(false) == 0
}
"#;

#[test]
fn break_or_continue_in_a_nested_while_condition_builds() {
    let file = source("loop_condition", LOOP_CONDITION);
    if ignite::load(&file, None).is_err() {
        return;
    }
    let failed = failures("loop_condition", &file);
    assert!(
        failed.is_empty(),
        "loop_condition:\n{}",
        failed.join("\n\n")
    );
}

// ---- `break` with no loop around it ------------------------------------------

const OUTSIDE_LOOP: &str = r#"package sc_adversary.outside_loop

fn or_break(p: Bool) -> Bool {
  p || (break)
}

fn condition_break(p: Bool) -> Int {
  var i = 0
  while i < 5 && (p || (break)) {
    i = i + 1
  }
  i
}

test "an operand that would break is not reached" {
  assert or_break(true)
  assert condition_break(true) == 5
}
"#;

#[test]
fn break_outside_a_loop_is_refused_or_builds() {
    let file = source("outside_loop", OUTSIDE_LOOP);
    if ignite::load(&file, None).is_err() {
        return;
    }
    let failed = failures("outside_loop", &file);
    assert!(
        failed.is_empty(),
        "outside_loop checks, so it must build:\n{}",
        failed.join("\n\n")
    );
}

// ---- positions that agree today ----------------------------------------------

const GUARDS: &str = r#"package sc_adversary.guards

type Point { x: Int, y: Int }

fn chain3(a: Bool, b: Bool, c: Bool) -> Int {
  if (a || (return 1)) && (b || (return 2)) && (c || (return 3)) { 4 } else { 5 }
}

fn chain_last(a: Bool, b: Bool) -> Int {
  if a && b && (return 6) { 7 } else { 8 }
}

fn chain_or(a: Bool, b: Bool) -> Int {
  if a || b || (return 9) { 10 } else { 11 }
}

fn chain_deep(a: Bool, b: Bool) -> Int {
  if a && (b || (a && (return 12))) { 13 } else { 14 }
}

fn try_right(r: Result<Bool, Int>, p: Bool) -> Result<Bool, Int> {
  Ok(p && r?)
}

fn try_left(r: Result<Bool, Int>, p: Bool) -> Result<Bool, Int> {
  Ok(r? || p)
}

fn if_arms_return(p: Bool, q: Bool) -> Int {
  if p && (if q { return 1 } else { return 2 }) { 3 } else { 4 }
}

fn match_arms_return(p: Bool, q: Bool) -> Int {
  if p || (match q { true => return 5, false => return 6 }) { 7 } else { 8 }
}

fn mapped(xs: List<Bool>) -> List<Bool> {
  xs.map(|q| q || (return false))
}

fn closure_left(p: Bool) -> Bool {
  let f: fn(Bool) -> Bool = |q| (return q) || p
  f(false)
}

fn closure_right(p: Bool) -> Bool {
  let f: fn(Bool) -> Bool = |q| q && (return true)
  f(p)
}

fn add_left(a: Int, x: Int) -> Int {
  (return a) + x
}

fn add_right(a: Int, x: Int) -> Int {
  x + (return a)
}

fn shl_left(a: Int, x: Int) -> Int {
  (return a) << x
}

fn u32_add(a: U32, b: U32) -> U32 {
  (return a) + b
}

fn text_cat(a: Text, b: Text) -> Text {
  "<" + (return a) + b + ">"
}

fn eq_right(a: Bool, p: Bool) -> Bool {
  p == (return a)
}

fn index(l: List<Int>) -> Int {
  l[return 7]
}

fn tuple(a: Int) -> Int {
  let t = (return a, 2)
  t.1
}

fn record(a: Int) -> Int {
  let p = Point { x = return a, y = 1 }
  p.y
}

fn if_condition(a: Int) -> Int {
  if (return a) { 1 } else { 2 }
}

fn scrutinee(a: Int) -> Int {
  match (return a) {
    _ => 1
  }
}

fn let_sum(a: Int) -> Int {
  let y = (return a) + 1
  y
}

fn wrapped(a: Int) -> Option<Int> {
  Some(return Some(a))
}

test "chains of three with a returning operand in each place" {
  assert chain3(false, true, true) == 1
  assert chain3(true, false, true) == 2
  assert chain3(true, true, false) == 3
  assert chain3(true, true, true) == 4
  assert chain_last(true, true) == 6
  assert chain_last(true, false) == 8
  assert chain_or(false, false) == 9
  assert chain_or(false, true) == 10
  assert chain_deep(true, false) == 12
  assert chain_deep(true, true) == 13
  assert chain_deep(false, true) == 14
}

test "a try inside an operand" {
  assert try_right(Ok(true), true) == Ok(true)
  assert try_right(Err(1), true) == Err(1)
  assert try_right(Err(1), false) == Ok(false)
  assert try_left(Ok(false), true) == Ok(true)
  assert try_left(Err(2), true) == Err(2)
}

test "a right operand whose every arm returns" {
  assert if_arms_return(true, true) == 1
  assert if_arms_return(true, false) == 2
  assert if_arms_return(false, true) == 4
  assert match_arms_return(false, true) == 5
  assert match_arms_return(false, false) == 6
  assert match_arms_return(true, false) == 7
}

test "returns inside closures" {
  assert mapped([true, false]) == [true, false]
  assert closure_left(true) == false
  assert closure_right(true) == true
  assert closure_right(false) == false
}

test "a test returns from an assert operand" {
  assert false || (return)
  assert false
}

test "a test returns from a let operand" {
  let v = true && (return)
  assert v == false
  assert false
}

test "a returning operand of other operators and positions" {
  assert add_left(1, 2) == 1
  assert add_right(1, 2) == 1
  assert shl_left(3, 1) == 3
  assert u32_add(1.wrap_u32(), 2.wrap_u32()) == 1.wrap_u32()
  assert text_cat("a", "b") == "a"
  assert eq_right(true, false) == true
  assert index([1, 2]) == 7
  assert tuple(3) == 3
  assert record(4) == 4
  assert if_condition(5) == 5
  assert scrutinee(6) == 6
  assert let_sum(8) == 8
  assert wrapped(9) == Some(9)
}
"#;

#[test]
fn returning_operands_in_other_positions_agree() {
    assert_agree("guards", GUARDS);
}
