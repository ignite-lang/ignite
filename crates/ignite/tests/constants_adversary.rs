//! Adversary (story `rust-target-constants-built-once`, unit `constants`):
//! programs that use a constant built once (`docs/slice.md` section 6,
//! "Constants") in positions `constants::` does not cover, run by the
//! evaluator and both targets, and reads of such a constant counted
//! against the sentence "Such a read allocates nothing".

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::borrowed_reads_adversary::{agree, evaluator_failures, source};
use crate::common::Target;
use crate::todo;

/// A constant of fn type given to a parameter its callee only calls, a
/// lent `&dyn Fn` (`docs/slice.md` 20.3.7). At `1febe156` the argument
/// was `&*F()`, `F()` an `Arc<dyn Fn>`; with `F()` a `&'static Arc<..>`
/// the same text is a `&Arc<dyn Fn>`, which rustc refuses (E0277).
const FN_LENT: &str = r#"package fn_constant_lent

const F: fn(Int) -> Int = |x: Int| x + 1
const G: fn(Int) -> Int = double

fn double(n: Int) -> Int = n * 2

fn apply(f: fn(Int) -> Int, x: Int) -> Int = f(x)

test "a fn constant is lent to a parameter that only calls it" {
  assert apply(F, 3) == 4
  assert apply(G, 4) == 8
}
"#;

#[test]
fn adversary_a_fn_constant_lent_to_a_parameter_that_only_calls_it() {
    agree("fn_constant_lent", FN_LENT, 1);
}

/// Every change the language has, made to a copy of a constant (a `var`,
/// a by-value parameter, a record field, a list element, an enum
/// payload, a closure, `send`, `spawn`) or to the constant itself; the
/// constant reads its declared value afterwards. Both targets pass these
/// tests (`ignite test --target rust` and `--target llvm`, checked by
/// hand at `a79c9b96`); this runs the evaluator, the reference.
const CHANGES: &str = r#"package constant_changes

type Inner {
  name: Text
  tags: List<Text>
}

type Holder {
  inner: Inner
  items: List<Inner>
  names: List<Text>
}

enum Shape {
  Dot
  Words(List<Text>)
  Boxed(Inner)
}

const T: Text = "hello"
const B: Bytes = b"bytes"
const LI: List<Int> = [1, 2, 3]
const LT: List<Text> = ["a", "b", "c"]
const LL: List<List<Int>> = [[1], [2, 3]]
const LR: List<Inner> = [Inner { name = "x", tags = ["t"] }]
const M: Map<Text, Int> = { a = 1, b = 2 }
const R: Inner = Inner { name = "in", tags = ["p", "q"] }
const H: Holder = Holder { inner = Inner { name = "deep", tags = ["d"] }, items = [Inner { name = "i0", tags = ["e"] }], names = ["n"] }
const SH: Shape = Words(["w1", "w2"])
const BX: Shape = Boxed(Inner { name = "bx", tags = ["b1"] })
const TUP: (Text, List<Int>) = ("tup", [9])

fn grow_list(l: List<Int>) -> Int {
  l.push(4)
  l[0] = 100
  l.len() + l[0]
}

fn grow_inner(r: Inner) -> Int {
  r.tags.push("z")
  r.tags[0] = "changed"
  r.tags.clear()
  r.tags.len()
}

fn grow_holder(h: Holder) -> Int {
  h.inner.tags.push("z")
  h.items[0].tags.push("y")
  h.names.append(["m"])
  h.items.len() + h.items[0].tags.len() + h.names.len()
}

fn grow_map(m: Map<Text, Int>) -> Int {
  m.put("c", 3)
  m.put("a", 10)
  m.remove("b")
  m.len()
}

fn words_len(s: Shape) -> Int = match s {
  Words(l) => {
    var m = l
    m.push("x")
    m.len()
  }
  Boxed(i) => {
    var j = i
    j.tags.push("y")
    j.tags.len()
  }
  Dot => 0
}

fn changer(l: List<Text>, out: Sender<Int>) -> Result<Unit, IoError> {
  l.push("spawned")
  l[0] = "q"
  out.send(l.len())
  Ok(())
}

fn holder_changer(h: Holder, out: Sender<Int>) -> Result<Unit, IoError> {
  h.inner.tags.clear()
  h.items[0].tags.push("w")
  out.send(h.items[0].tags.len())
  Ok(())
}

fn check_all() -> Bool {
  T == "hello" && B == b"bytes" && LI == [1, 2, 3] && LT == ["a", "b", "c"] && LL == [[1], [2, 3]] && LR[0].tags == ["t"] && M.len() == 2 && M.get("a") == Some(1) && R.tags == ["p", "q"] && R.name == "in" && H.inner.tags == ["d"] && H.items[0].tags == ["e"] && H.names == ["n"] && TUP.1 == [9]
}

fn shape_ok() -> Bool {
  let a = match SH {
    Words(l) => l == ["w1", "w2"]
    _ => false
  }
  let b = match BX {
    Boxed(i) => i.tags == ["b1"]
    _ => false
  }
  a && b
}

test "var then every mutation" {
  var t = T
  t = t + "!"
  var b = B
  b.push(1)
  b.append(B)
  b[0] = 0
  var li = LI
  li.push(4)
  li[0] = 9
  li.clear()
  var lt = LT
  lt.append(LT)
  lt[1] = "z"
  var ll = LL
  ll[1].push(4)
  ll[0] = [7]
  var lr = LR
  lr[0].tags.push("u")
  var m = M
  m.put("c", 3)
  m.remove("a")
  var r = R
  r.tags.push("x")
  r.tags[0] = "z"
  r.name = "changed"
  var h = H
  h.inner.tags.push("x")
  h.inner.name = "w"
  h.items[0].tags.push("y")
  h.items[0].tags[0] = "f"
  h.names.clear()
  var tup = TUP
  tup.1.push(10)
  assert t == "hello!"
  assert b.len() == 11
  assert li.len() == 0
  assert lt.len() == 6
  assert ll == [[7], [2, 3, 4]]
  assert lr[0].tags.len() == 2
  assert m.len() == 2
  assert r.tags == ["z", "q", "x"]
  assert h.items[0].tags == ["f", "y"]
  assert tup.1 == [9, 10]
  assert check_all()
}

test "by-value parameter changed by the callee" {
  assert grow_list(LI) == 104
  assert grow_inner(R) == 0
  assert grow_inner(LR[0]) == 0
  assert grow_holder(H) == 5
  assert grow_map(M) == 2
  assert words_len(SH) == 3
  assert words_len(BX) == 2
  assert check_all()
  assert shape_ok()
}

test "record and list and payload holding the constant" {
  var h = Holder { inner = R, items = LR, names = LT }
  h.inner.tags.push("k")
  h.items[0].tags.push("k")
  h.names[0] = "k"
  var l = [R, R]
  l[0].tags.push("k")
  l[1].tags.clear()
  var ls = [LT]
  ls[0].push("k")
  var w = Words(LT)
  let n = match w {
    Words(x) => {
      var y = x
      y.push("k")
      y.len()
    }
    _ => 0
  }
  assert n == 4
  assert l[0].tags.len() == 3
  assert check_all()
}

test "closure captures and changes" {
  let f = |x: Int| {
    var c = LI
    c.push(x)
    c[0] = x
    c.len()
  }
  let g = |s: Text| {
    var r = R
    r.tags.push(s)
    r.tags.len()
  }
  assert f(5) == 4
  assert f(6) == 4
  assert g("a") == 3
  assert g("b") == 3
  assert check_all()
}

test "send and spawn" {
  let (tx, rx) = Channel.bounded(4)
  spawn changer(LT, tx)
  assert rx.receive() == Some(4)
  spawn holder_changer(H, tx)
  assert rx.receive() == Some(2)
  let (ltx, lrx) = Channel.bounded(2)
  assert ltx.send(H)
  var got = match lrx.receive() {
    Some(h) => h
    None => H
  }
  got.items[0].tags.push("sent")
  got.names.push("sent")
  assert got.items[0].tags.len() == 2
  assert check_all()
}

test "change in place of the constant itself" {
  T.concat("x")
  B.push(1)
  B[0] = 0
  LI.push(4)
  LI[0] = 9
  LI.clear()
  LT.append(LT)
  LT[1] = "z"
  LL[1].push(4)
  LL[0] = [7]
  LR[0].tags.push("u")
  LR[0].tags[0] = "v"
  M.put("c", 3)
  M.remove("a")
  R.tags.push("x")
  R.tags[0] = "z"
  R.tags.clear()
  H.inner.tags.push("x")
  H.items[0].tags.push("y")
  H.items[0].tags[0] = "f"
  H.names.clear()
  H.names.append(LT)
  TUP.1.push(10)
  assert check_all()
  assert shape_ok()
}
"#;

#[test]
fn adversary_every_change_of_a_copy_leaves_the_constant_on_the_evaluator() {
    let file = source("constant_changes", CHANGES);
    let failures = evaluator_failures(&file);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Four reads of a constant built once: a `Map` key (`m.get(KEY)`), the
/// argument of a builtin (`t.starts_with(KEY)`), a `for` over a `Map`
/// constant, and a field of an element of a record constant
/// (`NESTED.items[0].name.len()`). `docs/slice.md` section 6 says a read
/// "allocates nothing" and names `for` and "an element or field read in
/// place" among the reads.
const READS: &str = r#"package constant_read_counts

type Inner {
  name: Text
  tags: List<Text>
}

type Outer {
  items: List<Inner>
}

const KEY: Text = "bravo"
const AGES: Map<Text, Int> = { alpha = 1, bravo = 2, charlie = 3 }
const NESTED: Outer = Outer { items = [Inner { name = "first", tags = ["t"] }] }

fn key_round(m: Map<Text, Int>) -> Int = match m.get(KEY) {
  Some(n) => n
  None => 0
}

fn starts_round(t: Text) -> Int = if t.starts_with(KEY) { 1 } else { 0 }

fn map_round() -> Int {
  var n = 0
  for (k, v) in AGES {
    n = n + v
  }
  n
}

fn element_field_round() -> Int = NESTED.items[0].name.len()

fn measure(form: Text, n: Int) -> Int {
  let m: Map<Text, Int> = { bravo = 2 }
  var total = 0
  var i = 0
  while i < n {
    if form == "key" {
      total = total + key_round(m)
    } else if form == "starts" {
      total = total + starts_round("bravo!")
    } else if form == "map" {
      total = total + map_round()
    } else if form == "field" {
      total = total + element_field_round()
    }
    i = i + 1
  }
  total
}

test "the reads give their values" {
  assert measure("key", 2) == 4
  assert measure("starts", 2) == 2
  assert measure("map", 2) == 12
  assert measure("field", 2) == 10
}

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
  log("{arguments[0]} {arguments[1]}: {measure(arguments[0], number(arguments[1]))}")
  return Ok(Unit)
}

system ConstantReadCounts {
  resource command_line: CommandLine
  component counts: Counts { command_line = command_line, executable = "read-counts" }
}
"#;

/// Rounds counted after the first.
const ROUNDS: u64 = 1000;

/// The allocations of `binary form n` under memcheck (`total heap usage`),
/// as `constants::` counts them.
fn allocations(binary: &Path, form: &str, n: u64) -> u64 {
    let log = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "constants-adversary-{form}-{n}-{}.log",
        std::process::id()
    ));
    let status = Command::new("valgrind")
        .arg("--tool=memcheck")
        .arg(format!("--log-file={}", log.display()))
        .arg(binary)
        .args([form, &n.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run valgrind");
    assert!(status.success(), "{form} {n}: {status:?}");
    let text = std::fs::read_to_string(&log).expect("the valgrind log");
    let _ = std::fs::remove_file(&log);
    let usage = text
        .lines()
        .find_map(|line| line.split_once("total heap usage: ").map(|(_, rest)| rest))
        .unwrap_or_else(|| panic!("no heap usage in the valgrind log:\n{text}"));
    usage
        .split_whitespace()
        .next()
        .and_then(|n| n.replace(',', "").parse().ok())
        .unwrap_or_else(|| panic!("a count in {usage:?}"))
}

fn read_counts() -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("constants-adversary-source");
    std::fs::create_dir_all(&dir).expect("the source directory");
    let file = dir.join("constant_read_counts.ignite");
    std::fs::write(&file, READS).expect("write the source");
    todo::build(
        &file,
        "constants-adversary-Rust",
        Target::Rust,
        "read-counts",
    )
}

/// At `a79c9b96` each of the four reads allocates per round on the Rust
/// target: 1.001 (key), 1.006 (`starts_with` argument), 4.001 (`for` over
/// the map, 385 bytes) and 1.000 (field of an element); at `1febe156`
/// 1.006, 1.001, 4.006 and 7.005. The copies are the constant's
/// `N().clone()` in `&N().clone()`, `(N().clone())` and
/// `checked::at(..).name.clone()`.
#[test]
fn adversary_reading_a_constant_as_a_key_an_argument_a_map_or_an_element_field_allocates_nothing() {
    let binary = read_counts();
    let mut failures = Vec::new();
    // `field` copies the element's field it reads, as the same read of a
    // parameter or a local list does at `1febe156`
    // (`checked::at(&(*o.items), 0, ..).name.clone().len()`): one copy
    // per round, story `rust-target-read-only-bindings-borrow`.
    for (form, copies) in [("key", 0), ("starts", 0), ("map", 0), ("field", 1)] {
        let first = allocations(&binary, form, 1);
        let all = allocations(&binary, form, 1 + ROUNDS);
        let per_round = all.saturating_sub(first) as f64 / ROUNDS as f64;
        println!("Rust, {form}: {per_round:.3} allocations per round");
        if all.saturating_sub(first).abs_diff(copies * ROUNDS) >= 10 {
            failures.push(format!(
                "{form}: {per_round:.3} allocations per round after the first on the Rust \
                 target, {copies} expected (docs/slice.md section 6, \"Constants\")"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
