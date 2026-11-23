//! Adversary (story `rust-target-text-reads-borrow`, second unit): the
//! by-value parameters every caller gives up (`docs/slice.md` section 6,
//! "By-value parameters given by every caller"). The rule holds for the
//! program's own module, typed with `type_program`, because no other
//! module imports it; a sibling module is typed with `type_module` and
//! keeps its parameters lent.
//!
//! `ignite test` over several roots types each named file as a program,
//! and a root may be the sibling of another root of the same run. The
//! group keeps the first typing of a module path, so which typing the
//! shared module is generated from depends on the order of the files on
//! the command line, while the other root's calls are generated from the
//! sibling typing.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::common;

fn root() -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join("given-params-adversary")
}

/// Write `files` (name, text) into `root()/case` and return that
/// directory.
fn layout(case: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = root().join(case);
    std::fs::create_dir_all(&dir).expect("the case directory");
    for (name, text) in files {
        std::fs::write(dir.join(name), text).expect("write a source file");
    }
    dir
}

/// A program `shop` that imports `keep_all` from its sibling and reads
/// its list after the call: it lends the list.
const SHOP: &str = r#"package shop

import shop.cart.{keep_all}

fn caller() -> Int {
  let l = ["a"]
  let n = keep_all(l).len()
  n + l.len()
}

test "the program lends its list to the sibling" {
  assert caller() == 2
}
"#;

/// The sibling `shop.cart`: its one caller gives its list up, so typed as
/// a program `keep_all` takes the list by value, and typed as a sibling
/// it lends it.
const CART: &str = r#"package shop.cart

fn keep_all(l: List<Text>) -> List<Text> {
  var out: List<Text> = []
  for x in l {
    out.push(x)
  }
  out
}

test "the sibling gives its list up" {
  assert keep_all(["x", "y"]) == ["x", "y"]
}
"#;

/// `ignite test cart.ignite shop.ignite --target rust`: the sibling named
/// first, as a shell glob over the directory names it. Both roots share
/// one executable; the group keeps `shop.cart` as typed by
/// `type_program` (`keep_all(l: Vec<String>)`), and `shop` is typed
/// against the sibling typing, whose `keep_all` lends (`keep_all(&l)`).
/// Red at adca570e: rustc refuses the generated crate with E0308
/// (expected `Vec<String>`, found `&Vec<String>`). Both orders passed at
/// e48f64a5, where every module was typed with `type_module`.
#[test]
fn a_sibling_named_before_its_program_in_one_test_run_builds_on_rust() {
    let dir = layout(
        "sibling-first",
        &[("shop.ignite", SHOP), ("cart.ignite", CART)],
    );
    let out = root().join("out").join("sibling-first");
    let output = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .current_dir(&dir)
        .arg("test")
        .args(["cart.ignite", "shop.ignite", "--target", "rust", "--out"])
        .arg(&out)
        .arg("--runtime")
        .arg(common::runner_archive())
        .arg("--target-dir")
        .arg(out.join("cargo"))
        .output()
        .expect("run ignite test");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let errors: Vec<&str> = stderr
        .lines()
        .chain(stdout.lines())
        .filter(|line| {
            line.starts_with("error") || line.contains("expected") || line.contains("FAILED")
        })
        .collect();
    assert!(
        output.status.code() == Some(0) && stdout.contains("test result: ok. 2 passed; 0 failed"),
        "exit {:?}\n{}",
        output.status.code(),
        errors.join("\n")
    );
}

/// Guard: callers that give a list up on one path and keep it on
/// another, before a `break`, in a `match` arm, through a field of a
/// given record, through mutual recursion and as an element of a given
/// list. Each `keepN` has one caller, and each caller has a path that
/// moves its parameter (an owner of it), so the rule applies at every
/// call it can: `keep1`, `keep2`, `keep4`, `keep7`, `keep8`,
/// `keep_or_count`, `keep_on_break`, `through_field` and `rows` take
/// their parameter by value on the Rust target at adca570e. The
/// evaluator is the reference; both targets give its values.
const GIVEN_PATHS: &str = r#"package given_paths

type Shelf {
  items: List<Text>
}

fn shelve(l: List<Text>) -> Shelf = Shelf { items = l }

fn keep1(l: List<Text>) -> List<Text> {
  var out: List<Text> = []
  for x in l {
    out.push(x)
  }
  out
}

fn keep2(l: List<Text>) -> List<Text> {
  var out: List<Text> = []
  for x in l {
    out.push(x)
  }
  out
}

fn keep3(l: List<Text>) -> List<Text> {
  var out: List<Text> = []
  for x in l {
    out.push(x)
  }
  out
}

fn keep4(l: List<Text>) -> List<Text> {
  var out: List<Text> = []
  for x in l {
    out.push(x)
  }
  out
}

fn keep5(l: List<Text>) -> List<Text> {
  var out: List<Text> = []
  for x in l {
    out.push(x)
  }
  out
}

fn keep6(l: List<Text>) -> List<Text> {
  var out: List<Text> = []
  for x in l {
    out.push(x)
  }
  out
}

fn keep7(l: List<Text>) -> List<Text> {
  var out: List<Text> = []
  for x in l {
    out.push(x)
  }
  out
}

fn keep8(l: List<Text>) -> List<Text> {
  var out: List<Text> = []
  for x in l {
    out.push(x)
  }
  out
}

fn keep_or_count(l: List<Text>, flag: Bool) -> Int {
  if l.len() > 99 {
    return shelve(l).items.len()
  }
  if flag {
    return keep1(l).len()
  }
  l.len() * 100
}

fn keep_on_break(l: List<Text>) -> Int {
  if l.len() > 99 {
    return shelve(l).items.len()
  }
  var i = 0
  var n = 0
  while i < 5 {
    if i == 2 {
      n = keep2(l).len()
      break
    }
    i = i + 1
  }
  n * 10 + i
}

fn parsed(l: List<Text>) -> Result<List<Text>, Text> {
  if l.len() > 99 {
    return Ok(shelve(l).items)
  }
  if l.is_empty() {
    return Err("empty")
  }
  Ok(keep3(l))
}

fn tried(l: List<Text>) -> Result<Int, Text> {
  let r = parsed(l)?
  Ok(r.len())
}

fn through_field(s: Shelf) -> Int {
  if s.items.len() > 99 {
    let kept = s
    return kept.items.len()
  }
  keep4(s.items).len()
}

fn rec(l: List<Text>, n: Int) -> List<Text> {
  if l.len() > 99 {
    return shelve(l).items
  }
  if n == 0 {
    return keep5(l)
  }
  rec(l, n - 1)
}

fn ping(l: List<Text>, n: Int) -> List<Text> {
  if n <= 0 {
    return l
  }
  pong(l, n - 1)
}

fn pong(l: List<Text>, n: Int) -> List<Text> {
  if l.len() > 99 {
    return shelve(l).items
  }
  if n <= 0 {
    return keep6(l)
  }
  ping(l, n - 1)
}

fn in_arm(o: Option<Int>) -> Int {
  let l = ["a", "b"]
  match o {
    Some(n) => if keep7(l).len() > n { 1 } else { 2 }
    None => 0
  }
}

fn rows(ls: List<List<Text>>) -> Int {
  if ls.len() > 99 {
    return [ls].len()
  }
  var total = 0
  for l in ls {
    total = total + keep8(l).len()
  }
  total
}

test "given on one path, read on the other" {
  assert keep_or_count(["a"], true) == 1
  assert keep_or_count(["a", "b"], false) == 200
}

test "given before a break" {
  assert keep_on_break(["a", "b", "c"]) == 32
}

test "given behind a question mark" {
  assert tried(["a"]) == Ok(1)
  assert tried([]) == Err("empty")
}

test "a field of a given record" {
  assert through_field(shelve(["x", "y"])) == 2
}

test "recursion and mutual recursion" {
  assert rec(["a", "b"], 3) == ["a", "b"]
  assert ping(["p"], 3) == ["p"]
  assert pong(["q"], 4) == ["q"]
}

test "a call in a match arm" {
  assert in_arm(Some(1)) == 1
  assert in_arm(Some(5)) == 2
  assert in_arm(None) == 0
}

test "an element of a given list" {
  assert rows([["a"], ["b", "c"]]) == 3
}
"#;

#[test]
fn a_parameter_given_on_some_paths_keeps_the_evaluator_value_on_both_targets() {
    crate::borrowed_reads_adversary::agree("given_paths", GIVEN_PATHS, 7);
}
