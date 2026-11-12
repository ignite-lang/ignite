//! Story `rust-target-constants-built-once` (`docs/slice.md` section 6,
//! "Constants"): a module constant whose type is not `Copy` in Rust is
//! built once per process on the Rust target and read through a reference,
//! so a use that only reads it allocates nothing, and a use that needs an
//! owned value copies it once.
//!
//! [`READS`] reads a `Text`, a `Bytes`, a list-of-pairs and a record
//! constant in a loop. [`COUNTS`] appended to it is one executable,
//! `constant-counts FORM KEY N`, that runs `N` rounds of the form `FORM`
//! (`text`, `bytes`, `pairs`, `record`, `args`, `map`; `none` runs no
//! round). Its allocations at `N` = [`FIRST`] + [`ROUNDS`] less those at
//! `N` = [`FIRST`] over [`ROUNDS`] are the allocations of one round after
//! the first, counted with valgrind memcheck's `total heap usage` line as
//! `shared_lists::` and `value_copies::` count them. [`OWNED`] and
//! [`POSITIONS`] are the owned uses and every position a constant stands
//! in; each program's inline tests run on the evaluator and both targets
//! (`borrowed_reads_adversary::agree`).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::borrowed_reads_adversary::{agree, evaluator_failures, source, target_failure};
use crate::common::Target;
use crate::todo;

/// Rounds before the counted ones, which build the constants.
const FIRST: u64 = 1;

/// Rounds counted.
const ROUNDS: u64 = 1000;

/// Fewer allocations than this over [`ROUNDS`] rounds are none per round.
const STRAY: u64 = 10;

/// The forms of [`READS`] and the allocations one round of each makes on
/// the Rust target. `field` (a field of an element, `PAIRS[i].1.len()`)
/// and `interp` (an element interpolated, `"{BOUNDS.tags[i]}!"`) copy the
/// text they read, as the same read of a local list or a parameter does
/// (`story:rust-target-read-only-bindings-borrow`); `interp` also
/// allocates the text it builds.
const FORMS: [(&str, u64); 8] = [
    ("text", 0),
    ("bytes", 0),
    ("pairs", 0),
    ("record", 0),
    ("args", 0),
    ("map", 0),
    ("field", 1),
    ("interp", 2),
];

/// Constants read in a loop, one form per kind of read.
const READS: &str = r#"package constant_reads

import std.text.{contains, ends_with}

type Bounds {
  depth: Int
  name: Text
  tags: List<Text>
}

const DIGITS: Text = "0123456789abcdef"
const MAGIC: Bytes = b"IGNITE-MAGIC"
const PAIRS: List<(Text, Text)> = [("a", "alpha"), ("b", "bravo"), ("c", "charlie"), ("d", "delta"), ("e", "echo"), ("f", "foxtrot"), ("g", "golf"), ("h", "hotel"), ("i", "india")]
const BOUNDS: Bounds = Bounds { depth = 3, name = "bounds", tags = ["x", "y"] }
const KEY: Text = "bounds"
const NEEDLE: Bytes = b"MAGIC"
const AGES: Map<Text, Int> = { ada = 36, bounds = 3 }

fn text_round(i: Int) -> Int {
  let digits = DIGITS.to_bytes()
  digits.at(i % 16).to_int() + DIGITS.len()
}

fn bytes_round(i: Int) -> Int = MAGIC.at(i % MAGIC.len()).to_int() + MAGIC.len()

fn find(key: Text) -> Int {
  for (k, v) in PAIRS {
    if k == key {
      return v.len()
    }
  }
  0
}

fn pairs_round(key: Text) -> Int = find(key) + PAIRS.len()

fn weigh(b: Bounds) -> Int = b.depth + b.name.len() + b.tags.len()

fn record_round(i: Int) -> Int = weigh(BOUNDS) + BOUNDS.depth + BOUNDS.tags[i % 2].len()

fn args_round(m: Map<Text, Int>, t: Text) -> Int {
  var n = 0
  if m.contains(KEY) {
    n = n + 1
  }
  n = n + match m.get(BOUNDS.name) {
    Some(v) => v
    None => 0
  }
  if t.starts_with(KEY) {
    n = n + 1
  }
  if ends_with(t, BOUNDS.name) {
    n = n + 1
  }
  if contains(t, KEY) {
    n = n + 1
  }
  n + match MAGIC.index_of(NEEDLE, 0) {
    Some(i) => i
    None => 0
  }
}

fn map_round() -> Int {
  var n = 0
  for (k, v) in AGES {
    n = n + v + k.len()
  }
  n
}

fn field_round(i: Int) -> Int = PAIRS[i % 9].1.len()

fn interp_round(i: Int) -> Int = "{BOUNDS.tags[i % 2]}!".len()

fn measure(form: Text, key: Text, n: Int) -> Int {
  var total = 0
  var i = 0
  let m: Map<Text, Int> = { bounds = 2 }
  if form == "text" {
    while i < n {
      total = total + text_round(i)
      i = i + 1
    }
  } else if form == "bytes" {
    while i < n {
      total = total + bytes_round(i)
      i = i + 1
    }
  } else if form == "pairs" {
    while i < n {
      total = total + pairs_round(key)
      i = i + 1
    }
  } else if form == "record" {
    while i < n {
      total = total + record_round(i)
      i = i + 1
    }
  } else if form == "args" {
    while i < n {
      total = total + args_round(m, "the bounds")
      i = i + 1
    }
  } else if form == "map" {
    while i < n {
      total = total + map_round()
      i = i + 1
    }
  } else if form == "field" {
    while i < n {
      total = total + field_round(i)
      i = i + 1
    }
  } else if form == "interp" {
    while i < n {
      total = total + interp_round(i)
      i = i + 1
    }
  }
  total
}

test "the constants read in a loop" {
  assert measure("text", "e", 3) == 195
  assert measure("bytes", "e", 3) == 258
  assert measure("pairs", "e", 3) == 39
  assert measure("record", "e", 3) == 45
  assert measure("args", "e", 3) == 36
  assert measure("map", "e", 3) == 144
  assert measure("field", "e", 3) == 17
  assert measure("interp", "e", 3) == 6
  assert measure("none", "e", 3) == 0
}
"#;

/// The counting executable appended to [`READS`].
const COUNTS: &str = r#"
type Counts {
  command_line: CommandLine
  executable: Text
}

fn number(t: Text) -> Int = match Int.parse(t) {
  Some(n) => n
  None => 0
}

fn Counts.main(self: Counts) -> Result<Unit, IoError> {
  let arguments = self.command_line.arguments()
  log("{arguments[0]} {arguments[2]}: {measure(arguments[0], arguments[1], number(arguments[2]))}")
  return Ok(Unit)
}

system ConstantCounts {
  resource command_line: CommandLine
  component counts: Counts { command_line = command_line, executable = "constant-counts" }
}
"#;

/// The uses that need an owned value: each copies the constant once, and
/// a later change is seen by no other use.
const OWNED: &str = r#"package owned_constants

type Bounds {
  depth: Int
  name: Text
  tags: List<Text>
}

type Holder {
  pairs: List<(Text, Text)>
  magic: Bytes
}

const DIGITS: Text = "0123456789abcdef"
const MAGIC: Bytes = b"IGNITE-MAGIC"
const PAIRS: List<(Text, Text)> = [("a", "alpha"), ("b", "bravo"), ("c", "charlie")]
const BOUNDS: Bounds = Bounds { depth = 3, name = "bounds", tags = ["x", "y"] }

fn pushed(l: List<(Text, Text)>) -> Int {
  l.push(("z", "zulu"))
  l.len()
}

fn magic() -> Bytes = MAGIC

fn bounds() -> Bounds {
  return BOUNDS
}

fn grow(l: List<(Text, Text)>, done: Sender<Int>) -> Result<Unit, IoError> {
  var mine = l
  mine.push(("t", "task"))
  mine[0] = ("q", "quebec")
  done.send(mine.len() + mine[0].1.len())
  Ok(())
}

test "a by-value argument copies the constant once" {
  assert pushed(PAIRS) == 4
  assert pushed(PAIRS) == 4
  assert PAIRS.len() == 3
}

test "a var initialised from a constant and changed leaves the constant" {
  var t = DIGITS
  t = t + "!"
  var b = BOUNDS
  b.tags.push("z")
  b.name = "changed"
  var l = PAIRS
  l[0] = ("x", "xray")
  assert t == "0123456789abcdef!"
  assert DIGITS == "0123456789abcdef"
  assert b.tags.len() == 3
  assert BOUNDS.tags.len() == 2
  assert BOUNDS.name == "bounds"
  assert PAIRS[0] == ("a", "alpha")
}

test "a returned constant is a copy" {
  var m = magic()
  m.push(33)
  var b = bounds()
  b.depth = 9
  b.tags.clear()
  assert m.len() == 13
  assert MAGIC.len() == 12
  assert magic().len() == 12
  assert bounds().depth == 3
  assert BOUNDS.tags == ["x", "y"]
}

test "a constant stored in a record is a copy" {
  var h = Holder { pairs = PAIRS, magic = MAGIC }
  h.pairs.push(("d", "delta"))
  h.magic.push(0)
  assert h.pairs.len() == 4
  assert PAIRS.len() == 3
  assert MAGIC.len() == 12
}

test "a constant pushed into a list is a copy" {
  var l = [BOUNDS]
  l.push(BOUNDS)
  l[0] = Bounds { depth = 9, ..BOUNDS }
  l[1].tags.push("w")
  var t = [DIGITS]
  t[0] = t[0] + "?"
  assert l[0].depth == 9
  assert BOUNDS.depth == 3
  assert BOUNDS.tags.len() == 2
  assert t[0] == "0123456789abcdef?"
  assert DIGITS.len() == 16
}

test "a constant captured by a closure is a copy" {
  let f = |x: Byte| {
    var m = MAGIC
    m.push(x)
    m.len()
  }
  assert f(1) == 13
  assert f(2) == 13
  let g = |n: Int| BOUNDS.depth + n
  assert g(1) == 4
  assert MAGIC.len() == 12
}

test "a constant sent on a channel is a copy" {
  let (tx, rx) = Channel.bounded(2)
  assert tx.send(PAIRS)
  assert tx.send(PAIRS)
  var first = match rx.receive() {
    Some(l) => l
    None => []
  }
  first.push(("e", "echo"))
  let second = match rx.receive() {
    Some(l) => l
    None => []
  }
  assert first.len() == 4
  assert second.len() == 3
  assert PAIRS.len() == 3
}

test "a constant given to spawn is a copy" {
  let (done, results) = Channel.bounded(2)
  spawn grow(PAIRS, done)
  spawn grow(PAIRS, done)
  assert results.receive() == Some(10)
  assert results.receive() == Some(10)
  assert PAIRS.len() == 3
  assert PAIRS[0] == ("a", "alpha")
}
"#;

/// Every position a constant stands in, with constants of a std module
/// and of the sibling module [`TABLES`].
const POSITIONS: &str = r#"package constant_positions

import std.encoding.{STANDARD}
import constant_positions.tables.{FIRST, GREEK, Letter}

type Bounds {
  depth: Int
  name: Text
  tags: List<Text>
}

type Pair<A, B> {
  first: A
  second: B
}

type Table = List<(Text, Text)>

enum Shape {
  Dot
  Line(Int)
  Words(List<Text>)
}

const DIGITS: Text = "0123456789abcdef"
const MAGIC: Bytes = b"IGNITE-MAGIC"
const NAMES: List<Text> = ["ada", "bob", "cy"]
const BOUNDS: Bounds = Bounds { depth = 3, name = "bounds", tags = ["x", "y"] }
const SHAPE: Shape = Words(["p", "q", "r"])
const LINE: Shape = Line(7)
const MAYBE: Option<Text> = Some("here")
const NOTHING: Option<Bounds> = None
const AGES: Map<Text, Int> = { ada = 36, bob = 41 }
const PAIR: Pair<Text, Int> = Pair { first = "one", second = 1 }
const TABLE: Table = [("a", "alpha"), ("b", "bravo")]
const BOTH: Text = DIGITS + "!" + BOUNDS.name
const NESTED: List<Bounds> = [BOUNDS, Bounds { depth = 1, name = "inner", tags = [] }]
const LOUD: Text = "LOUD"

fn weigh(b: Bounds) -> Int = b.depth + b.name.len() + b.tags.len()

fn count(l: List<Text>) -> Int = l.len()

fn describe(s: Shape) -> Text = match s {
  Dot => "dot"
  Line(n) => "line {n}"
  Words(w) => "words {w.len()}"
}

fn volume(t: Text) -> Int = match t {
  LOUD => 2
  _ => 1
}

fn sum_bytes() -> Int {
  var n = 0
  for b in MAGIC {
    n = n + b.to_int()
  }
  n
}

fn joined() -> Text {
  var out = ""
  for name in NAMES {
    out = out + name
  }
  out
}

fn lookup(key: Text) -> Text {
  for (k, v) in TABLE {
    if k == key {
      return v
    }
  }
  ""
}

fn greek(key: Text) -> Text {
  for (k, v) in GREEK {
    if k == key {
      return v
    }
  }
  ""
}

fn age_total() -> Int {
  var n = 0
  for (k, v) in AGES {
    n = n + v + k.len()
  }
  n
}

fn reader(names: Sender<Int>) -> Result<Unit, IoError> {
  var n = 0
  for name in NAMES {
    n = n + name.len()
  }
  names.send(n + BOUNDS.depth + weigh(BOUNDS))
  Ok(())
}

test "a constant is a read-only argument" {
  assert weigh(BOUNDS) == 11
  assert count(NAMES) == 3
  assert describe(SHAPE) == "words 3"
  assert describe(LINE) == "line 7"
}

test "a constant is a method receiver" {
  assert DIGITS.len() == 16
  assert NAMES[1] == "bob"
  assert MAGIC[0].to_int() == 73
  assert MAGIC.slice(0, 6).to_text() == "IGNITE"
  assert DIGITS.starts_with("0123")
  assert DIGITS.to_bytes()[10].to_int() == 97
  assert NAMES.slice(1, 3) == ["bob", "cy"]
}

test "a constant is iterated" {
  assert sum_bytes() == 846
  assert joined() == "adabobcy"
  assert lookup("b") == "bravo"
  assert age_total() == 83
}

test "a constant is matched" {
  let n = match MAYBE {
    Some(t) => t.len()
    None => 0
  }
  assert n == 4
  let m = match NOTHING {
    Some(b) => b.depth
    None => 0 - 1
  }
  assert m == 0 - 1
  let w = match SHAPE {
    Words(l) => l[2]
    _ => ""
  }
  assert w == "r"
  assert volume("LOUD") == 2
  assert volume("soft") == 1
}

test "a field of a record constant is read" {
  assert BOUNDS.name == "bounds"
  assert BOUNDS.tags[1] == "y"
  assert BOUNDS.tags.len() == 2
  assert NESTED[1].name == "inner"
  assert PAIR.first == "one"
  assert PAIR.second == 1
}

test "a constant is interpolated and compared" {
  assert "{DIGITS}-{BOUNDS.depth}" == "0123456789abcdef-3"
  assert DIGITS == "0123456789abcdef"
  assert DIGITS != BOTH
  assert BOTH == "0123456789abcdef!bounds"
  assert TABLE[0] == ("a", "alpha")
  assert MAYBE == Some("here")
  assert NOTHING == None
  assert AGES.get("bob") == Some(41)
  assert NESTED[0].depth == BOUNDS.depth
}

test "a constant of another module is read" {
  assert STANDARD.len() == 64
  assert STANDARD.to_bytes()[0].to_int() == 65
  assert greek("b") == "beta"
  assert GREEK.len() == 3
  let l: Letter = FIRST
  assert l.name == "alpha"
}

test "a constant is read from two tasks" {
  let (tx, rx) = Channel.bounded(2)
  spawn reader(tx)
  spawn reader(tx)
  assert rx.receive() == Some(22)
  assert rx.receive() == Some(22)
}

test "a change in place of a constant changes a copy" {
  MAGIC.push(1)
  NAMES.clear()
  NAMES[0] = "zed"
  BOUNDS.tags.push("w")
  BOUNDS.tags[0] = "q"
  AGES.put("cy", 9)
  assert MAGIC.len() == 12
  assert NAMES == ["ada", "bob", "cy"]
  assert BOUNDS.tags == ["x", "y"]
  assert AGES.len() == 2
}
"#;

/// The sibling module of [`POSITIONS`].
const TABLES: &str = r#"package constant_positions.tables

type Letter {
  name: Text
  rank: Int
}

const GREEK: List<(Text, Text)> = [("a", "alpha"), ("b", "beta"), ("g", "gamma")]
const FIRST: Letter = Letter { name = "alpha", rank = 1 }
"#;

/// The counting executable, built for `target`.
fn counting(target: Target) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("constants-source-{target:?}"));
    std::fs::create_dir_all(&dir).expect("the source directory");
    let file = dir.join("constant_reads.ignite");
    std::fs::write(&file, format!("{READS}{COUNTS}")).expect("write the source");
    todo::build(
        &file,
        &format!("constants-{target:?}"),
        target,
        "constant-counts",
    )
}

/// The allocations and bytes of `binary form key n` run to its end under
/// memcheck (its `total heap usage` line), and its log line.
fn heap(binary: &Path, form: &str, n: u64) -> (u64, u64, String) {
    let log = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "constants-{}-{form}-{n}-{}.log",
        binary
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|p| p.to_str())
            .unwrap_or("run"),
        std::process::id()
    ));
    let output = Command::new("valgrind")
        .arg("--tool=memcheck")
        .arg(format!("--log-file={}", log.display()))
        .arg(binary)
        .args([form, "e", &n.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .expect("run valgrind");
    let line = String::from_utf8_lossy(&output.stderr).trim().to_string();
    assert!(
        output.status.success(),
        "{form} {n}: {:?}: {line}",
        output.status
    );
    let text = std::fs::read_to_string(&log).expect("the valgrind log");
    let _ = std::fs::remove_file(&log);
    let usage = text
        .lines()
        .find_map(|line| line.split_once("total heap usage: ").map(|(_, rest)| rest))
        .unwrap_or_else(|| panic!("no heap usage in the valgrind log:\n{text}"));
    let number = |part: &str| -> u64 {
        part.split_whitespace()
            .next()
            .and_then(|n| n.replace(',', "").parse().ok())
            .unwrap_or_else(|| panic!("a count in {usage:?}"))
    };
    let parts: Vec<&str> = usage.split(", ").collect();
    (number(parts[0]), number(parts[2]), line)
}

/// Allocations and bytes of one round of `form` after the first, and the
/// log line of the longer run.
fn per_round(binary: &Path, form: &str) -> (f64, f64, String) {
    let (first, first_bytes, _) = heap(binary, form, FIRST);
    let (all, all_bytes, line) = heap(binary, form, FIRST + ROUNDS);
    (
        all.saturating_sub(first) as f64 / ROUNDS as f64,
        all_bytes.saturating_sub(first_bytes) as f64 / ROUNDS as f64,
        line,
    )
}

/// Bullet 1 and 4 of the story: one round that reads a `Text`, a `Bytes`,
/// a list-of-pairs or a record constant allocates nothing after the
/// first round on the Rust target; nor does one (`args`) that gives
/// constants, and a field of one, to builtins and `std` fns that only
/// read them (`Map.contains`, `Map.get`, `starts_with`, `ends_with`,
/// `contains`, `Bytes.index_of`), nor one (`map`) that iterates a `Map`
/// constant; `field` and `interp` make the counts [`FORMS`] gives. A
/// round's allocations are whole, so
/// anything under [`STRAY`] over [`ROUNDS`] rounds is not one per round
/// (6 at `1febe156`, inferred to be the longer run's longer log line;
/// a round there made 3, 3, 38 and 16). The LLVM executable
/// prints the values the Rust one prints; its allocations per round are
/// printed, not held (a fix there is a story of its own).
#[test]
fn reading_a_constant_in_a_loop_allocates_nothing_per_round_on_the_rust_target() {
    let status = Command::new("valgrind")
        .args(["--tool=memcheck", "--log-file=/dev/null", "true"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap_or_else(|e| panic!("valgrind is needed to count allocations and did not run: {e}"));
    assert!(status.success(), "valgrind did not run `true`: {status:?}");
    let rust = counting(Target::Rust);
    let llvm = counting(Target::Llvm);
    let mut failures = Vec::new();
    for (form, expected) in FORMS {
        let (allocations, bytes, line) = per_round(&rust, form);
        println!("Rust, {form}: {allocations:.3} allocations and {bytes:.1} bytes per round");
        let counted = (allocations * ROUNDS as f64).round() as u64;
        if counted.abs_diff(expected * ROUNDS) >= STRAY {
            failures.push(format!(
                "{form}: {allocations:.3} allocations ({bytes:.1} bytes) per round after the \
                 first on the Rust target, {expected} expected"
            ));
        }
        let (llvm_allocations, llvm_bytes, llvm_line) = per_round(&llvm, form);
        println!(
            "LLVM, {form}: {llvm_allocations:.3} allocations and {llvm_bytes:.1} bytes per round"
        );
        if llvm_line != line {
            failures.push(format!("{form}: LLVM printed {llvm_line:?}, Rust {line:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The four constants give the same values on the evaluator and both
/// targets.
#[test]
fn the_four_constants_read_the_same_on_every_consumer() {
    agree("constant_reads", READS, 1);
}

/// Bullet 2: a by-value argument, a changed `var`, a returned constant,
/// one stored in a record, pushed into a list, captured by a closure,
/// sent on a channel and given to `spawn` each copy the constant, one
/// inline test per form, and no change reaches another use.
#[test]
fn a_use_that_needs_an_owned_value_copies_the_constant_on_every_consumer() {
    agree("owned_constants", OWNED, 8);
}

/// Every position a constant stands in compiles on the Rust target and
/// gives the evaluator's values on both targets: a read-only argument, a
/// method receiver, `for`, `match`, a constant pattern, a field of a
/// record constant, interpolation, comparison, a constant in another
/// constant's value, a generic and an aliased type, a shared list in a
/// field and a payload, a map, an `Option`, a std constant, a sibling
/// module's constant, a constant read from two tasks, and a change in
/// place of a constant, which changes a copy.
#[test]
fn a_constant_compiles_in_every_position_on_every_consumer() {
    let file = source("constant_positions", POSITIONS);
    std::fs::write(file.with_file_name("tables.ignite"), TABLES).expect("write the sibling");
    let mut failures = evaluator_failures(&file);
    for target in ["rust", "llvm"] {
        failures.extend(target_failure(&file, target, 9));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Constants of fn type, of this module and of a sibling module
/// ([`FN_MORE`]), in every position the language gives one; built once,
/// each is an `Arc<dyn Fn>` read through a `&'static`. A constant is not
/// called directly (E003: "bind it first") and fn values do not compare
/// (E045); `std` declares no constant of fn type.
const FN_POSITIONS: &str = r#"package fn_constants

import fn_constants.more.{TRIPLE}

type Holder {
  f: fn(Int) -> Int
  name: Text
}

const F: fn(Int) -> Int = |x: Int| x + 1
const G: fn(Int) -> Int = double

fn double(n: Int) -> Int = n * 2

fn apply(f: fn(Int) -> Int, x: Int) -> Int = f(x)

fn keep(f: fn(Int) -> Int) -> Holder = Holder { f = f, name = "kept" }

fn give() -> fn(Int) -> Int = F

fn give_other() -> fn(Int) -> Int = TRIPLE

test "a fn constant is lent to a parameter that only calls it" {
  assert apply(F, 3) == 4
  assert apply(G, 4) == 8
  assert apply(TRIPLE, 2) == 6
}

test "a fn constant is bound and called" {
  let f = F
  let t = TRIPLE
  assert f(1) + t(1) == 5
}

test "a fn constant is a record field" {
  let h = Holder { f = F, name = "h" }
  let k = Holder { f = TRIPLE, name = "k" }
  let hf = h.f
  let kf = k.f
  assert hf(1) + kf(1) == 5
}

test "a fn constant is passed by value" {
  let h = keep(G)
  let k = keep(TRIPLE)
  let hf = h.f
  let kf = k.f
  assert hf(3) + kf(3) == 15
}

test "a fn constant is returned" {
  let f = give()
  let t = give_other()
  assert f(9) + t(1) == 13
}

test "a fn constant is mapped and listed" {
  assert [1, 2].map(F) == [2, 3]
  assert [1, 2].map(TRIPLE) == [3, 6]
  let fs = [F, G, TRIPLE]
  var n = 0
  for f in fs {
    n = n + f(1)
  }
  assert n == 7
}

test "a fn constant is lent from a closure" {
  let c = |x: Int| apply(F, x) + apply(TRIPLE, x)
  assert apply(c, 1) == 5
}
"#;

/// The sibling module of [`FN_POSITIONS`].
const FN_MORE: &str = r#"package fn_constants.more

const TRIPLE: fn(Int) -> Int = |x: Int| x * 3
"#;

/// A constant of fn type compiles in every position on the Rust target
/// (lent to a parameter that only calls it: `&**F()`, the `dyn Fn` behind
/// the `&'static Arc`) and gives the evaluator's values on both targets,
/// one inline test per position.
#[test]
fn a_fn_constant_compiles_in_every_position_on_every_consumer() {
    let file = source("fn_constants", FN_POSITIONS);
    std::fs::write(file.with_file_name("more.ignite"), FN_MORE).expect("write the sibling");
    let mut failures = evaluator_failures(&file);
    for target in ["rust", "llvm"] {
        failures.extend(target_failure(&file, target, 7));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
