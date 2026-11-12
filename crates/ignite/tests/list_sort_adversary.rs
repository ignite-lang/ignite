//! Adversary cases for story `list-sort` (`docs/slice.md`, "A List is
//! sorted with sort and sort_by"): programs the unit's own tests do not
//! build, run by the evaluator and both targets where they can be.
//!
//! - [`NEVER`]: a key fn whose result type is `Never` (every path ends the
//!   process). The type checker counts `Never` among the types with an
//!   order (`ignite_types::ordered`), so the program checks; the Rust
//!   target builds it; the LLVM target must too.
//! - [`EDGES`]: the edges of every type with an order, the sizes around
//!   the runtime merge sort's widths, stability with groups of every size,
//!   and the value semantics of the receiver and the result, printed by
//!   the evaluator and both targets and pinned line by line.
//! - [`UNORDERED`]: one E045 for every element and key type without an
//!   order.
//! - [`INFERRED`] and [`ON_PARAMETER`]: the effects of a key fn are
//!   charged to a caller whose effects are inferred, and an unannotated
//!   closure parameter's effects are E017, as for `map`.
//! - [`KEYS`]: a failing key fn fails with one report on all three, and a
//!   key fn with an effect runs once per element, in order, on both targets.
//! - [`ALLOC`]: the Rust target's allocations of a `sort_by` of `Text`
//!   against the NN.2 sentence "copies each element once".

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use ignite_eval::{EvalError, Evaluator, Fuel};
use ignite_syntax::ast::{Decl, Program};
use ignite_types::QualName;

use crate::common::Target;
use crate::todo;

/// `program` written to `<tmp>/list-sort-adversary/<name>/<name>.ignite`.
fn source(name: &str, program: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("list-sort-adversary")
        .join(name);
    std::fs::create_dir_all(&dir).expect("the source directory");
    let file = dir.join(format!("{name}.ignite"));
    std::fs::write(&file, program).expect("write the program");
    file
}

/// Exit status, stdout and stderr of `binary args`.
fn run(binary: &Path, args: &[&str]) -> (Option<i32>, String, String) {
    let output = Command::new(binary)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("run {}: {e}", binary.display()));
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

// ---- a key fn that never returns -------------------------------------------

/// `sort_by` with a key fn whose every path ends the process: the key's
/// type is `Never`, which `ignite_types::ordered` accepts.
const NEVER: &str = r#"package never_key

fn never_match(xs: List<Int>) -> List<Int> = xs.sort_by(|x| match x {
  _ => Process.exit(3)
})

type Runner {
  executable: Text
}

fn Runner.main(self: Runner) -> Result<Unit, IoError> {
  let xs = never_match([2, 1])
  return Ok(Unit)
}

system NeverKey {
  component nevers: Runner { executable = "nevers" }
}
"#;

/// A program `ignite check` accepts builds on both targets: a `sort_by`
/// whose key fn never returns (its type `Never`, accepted as a key with an
/// order) builds on the Rust target and runs to the `Process.exit(3)` of the
/// first key, and must build and do the same on the LLVM target, whose
/// code generator has no order for `Never` (`glue::compare` panics).
#[test]
fn a_key_fn_that_never_returns_builds_on_llvm_as_on_rust() {
    let file = source("never_key", NEVER);
    let mut failures = Vec::new();
    for target in [Target::Rust, Target::Llvm] {
        let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("list-sort-adversary-never");
        let options = target.options(&out);
        let built = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ignite::build(&file, &options)
        }));
        let output = match built {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => {
                failures.push(format!("{target:?}: build failed: {e}"));
                continue;
            }
            Err(panic) => {
                let message = panic
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_default();
                failures.push(format!("{target:?}: the build panicked: {message}"));
                continue;
            }
        };
        let binary = output
            .executables
            .iter()
            .find(|path| path.file_stem().and_then(|s| s.to_str()) == Some("nevers"))
            .unwrap_or_else(|| panic!("no nevers in {:?}", output.executables));
        let (status, stdout, stderr) = run(binary, &[]);
        if status != Some(3) || !stdout.is_empty() {
            failures.push(format!(
                "{target:?}: exit {status:?}, stdout {stdout:?}, stderr {stderr:?}; expected exit 3"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// ---- edges printed by all three ----------------------------------------------

/// The edge programs: every fn without parameters is printed by `main`, in
/// declaration order, as the unit's fixture is.
const EDGES: &str = r#"package sort_edges

import std.rfc.{Level}

enum One { Only }

enum Two { Second, First }

enum Dir { North, East, South, West }

type Score = Int

type Box {
  items: List<Int>
  name: Text
}

fn word(n: Int) -> U32 = n.wrap_u32()

fn byte(n: Int) -> Byte = match n.to_byte() {
  Some(b) => b
  None => 0
}

fn seq(n: Int) -> List<Int> {
  var xs: List<Int> = List.with_capacity(n)
  var i = 0
  while i < n {
    xs.push((i * 7919) % 13)
    i = i + 1
  }
  xs
}

fn ascending(xs: List<Int>) -> Bool {
  var i = 1
  while i < xs.len() {
    if xs[i - 1] > xs[i] {
      return false
    }
    i = i + 1
  }
  true
}

fn grouped(n: Int, groups: Int) -> Bool {
  var xs: List<(Int, Int)> = List.empty()
  var i = 0
  while i < n {
    xs.push((i % groups, i))
    i = i + 1
  }
  let ys = xs.sort_by(|p| p.0)
  var j = 1
  while j < ys.len() {
    let a = ys[j - 1]
    let b = ys[j]
    if a.0 > b.0 || (a.0 == b.0 && a.1 > b.1) {
      return false
    }
    j = j + 1
  }
  ys.len() == n
}

fn box_list(n: Int) -> List<Box> = [Box { items = [3], name = "c" }, Box { items = [1, 2], name = "a" }, Box { items = [], name = "b" }]

fn param_sort(xs: List<Int>) -> (List<Int>, List<Int>) = (xs.sort(), xs)

fn make(n: Int) -> List<Int> = [9, 8, 7]

fn sizes() -> List<Int> {
  var bad: List<Int> = List.empty()
  for n in [0, 1, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17, 31, 32, 33, 63, 64, 65, 127, 128, 129, 1025] {
    if !ascending(seq(n).sort()) || seq(n).sort().len() != n {
      bad.push(n)
    }
  }
  bad
}

fn stable_sizes() -> List<(Int, Int)> {
  var bad: List<(Int, Int)> = List.empty()
  for n in [1, 2, 3, 5, 9, 17, 33, 65, 129, 257] {
    for groups in [1, 2, 3, 7, 64] {
      if !grouped(n, groups) {
        bad.push((n, groups))
      }
    }
  }
  bad
}

fn int_edges() -> List<Int> = [0, -9223372036854775807 - 1, 1, 9223372036854775807, -1, -9223372036854775807, 9223372036854775806].sort()

fn byte_edges() -> List<Byte> = [byte(255), byte(0), byte(128), byte(127)].sort()

fn words() -> List<U32> = [word(4294967295), word(2147483647), word(2147483648), word(0), word(1)].sort()

fn texts() -> List<Text> = ["\u{10FFFF}", "\u{FFFF}", "\u{7F}", "\u{80}", "aa", "a", "", "\u{E9}", "e\u{301}", "\u{1F600}"].sort()

fn byte_strings() -> List<Bytes> = [b"\xff\x00", b"\xff", b"\x00\xff", b"\x00", b"", b"\x7f", b"\x80"].sort()

fn ones() -> List<One> = [Only, Only].sort()

fn twos() -> List<Two> = [First, Second, First].sort()

fn dirs() -> List<Dir> = [West, North, South, East, West].sort()

fn std_levels() -> List<Level> = [Should, May, Must, MustNot].sort()

fn scores() -> List<Score> = [3, 1, 2].sort()

fn enum_then_byte() -> List<(Dir, Byte)> = [(East, byte(1)), (North, byte(9)), (East, byte(0)), (North, byte(2))].sort()

fn tuple_text_int() -> List<(Text, Int)> = [("b", 2), ("a", 3), ("b", 1), ("a", -3), ("", 0)].sort()

fn tuple3() -> List<(Bool, (Byte, (U32, Text)))> = [(true, (byte(1), (word(5), "x"))), (true, (byte(1), (word(4294967295), "a"))), (false, (byte(200), (word(0), "z"))), (true, (byte(0), (word(9), "q")))].sort()

fn padded() -> List<(Byte, Int, Bool, U32)> = [(byte(2), 5, true, word(1)), (byte(1), 9, false, word(3)), (byte(2), 5, false, word(2)), (byte(1), -9, true, word(0))].sort()

fn constant_key() -> List<Int> = [5, 3, 9, 1, 7, 2, 8].sort_by(|x| 0)

fn tuple_first_equal() -> List<(Int, Text)> = [(1, "z"), (1, "a"), (0, "m"), (1, "b")].sort_by(|p| p.0)

fn box_sort() -> (List<Text>, List<Text>) {
  let xs = box_list(0)
  var ys = xs.sort_by(|b| b.name)
  ys[0].items.push(99)
  (xs.map(|b| b.items.debug()), ys.map(|b| b.items.debug()))
}

fn bytes_values() -> (List<Bytes>, List<Bytes>) {
  let xs = [b"b", b"a"]
  var ys = xs.sort()
  ys[0].set(0, byte(122))
  (xs, ys)
}

fn mutate_after() -> (List<Text>, List<Text>) {
  var xs = ["b", "a", "c"]
  var ys = xs.sort()
  ys.push("d")
  ys[0] = "q"
  xs.push("e")
  (xs, ys)
}

fn reassign() -> List<Int> {
  var xs = [3, 2, 1]
  xs = xs.sort()
  xs.push(0)
  xs
}

fn nested_lists() -> (List<List<Text>>, List<Text>) {
  let ls = [["b", "a"], ["d", "c"]]
  let s = ls[1].sort()
  (ls, s)
}

fn by_param() -> (List<Int>, List<Int>) = param_sort([2, 1])

fn temp_sort() -> List<Int> = make(0).sort()

fn captured() -> List<Int> {
  let base = [3, 1, 2]
  let f = |n: Int| base.sort()[n]
  [f(0), f(1), f(2)]
}

fn captured_key() -> List<Int> {
  let weights = [10, 30, 20]
  [0, 1, 2].sort_by(|i| weights[i])
}

fn match_bind() -> List<Int> = match Some([4, 2, 3]) {
  Some(xs) => xs.sort()
  None => []
}

fn for_over_sorted() -> List<Int> {
  var out: List<Int> = List.empty()
  for x in [3, 1, 2].sort() {
    out.push(x * 10)
  }
  out
}

fn text_key_tuple() -> List<Int> = [1, 2, 3, 4].sort_by(|x| (x % 2 == 0, (x * 37 % 5).to_text()))

fn keys_sorted() -> List<Text> {
  var m: Map<Text, Int> = Map.empty()
  m.put("b", 1)
  m.put("a", 2)
  m.keys().sort()
}

type Printer {
  executable: Text
}

fn show(text: Text) -> Result<Unit, IoError> requires [stdout.write] {
  var bytes = text.to_bytes()
  bytes.push(10)
  return Stdout.write_all(bytes)
}

fn Printer.main(self: Printer) -> Result<Unit, IoError> {
  show(sizes().debug())?
  show(stable_sizes().debug())?
  show(int_edges().debug())?
  show(byte_edges().debug())?
  show(words().debug())?
  show(texts().debug())?
  show(byte_strings().debug())?
  show(ones().debug())?
  show(twos().debug())?
  show(dirs().debug())?
  show(std_levels().debug())?
  show(scores().debug())?
  show(enum_then_byte().debug())?
  show(tuple_text_int().debug())?
  show(tuple3().debug())?
  show(padded().debug())?
  show(constant_key().debug())?
  show(tuple_first_equal().debug())?
  show(box_sort().debug())?
  show(bytes_values().debug())?
  show(mutate_after().debug())?
  show(reassign().debug())?
  show(nested_lists().debug())?
  show(by_param().debug())?
  show(temp_sort().debug())?
  show(captured().debug())?
  show(captured_key().debug())?
  show(match_bind().debug())?
  show(for_over_sorted().debug())?
  show(text_key_tuple().debug())?
  show(keys_sorted().debug())?
  return Ok(Unit)
}

system SortEdges {
  component edgeprint: Printer { executable = "edgeprint" }
}
"#;

/// Every printed fn of [`EDGES`] and its debug text, worked out by hand
/// from the order `docs/slice.md` states.
const EDGES_EXPECTED: &[(&str, &str)] = &[
    ("sizes", "[]"),
    ("stable_sizes", "[]"),
    (
        "int_edges",
        "[-9223372036854775808, -9223372036854775807, -1, 0, 1, 9223372036854775806, 9223372036854775807]",
    ),
    ("byte_edges", "[0, 127, 128, 255]"),
    ("words", "[0, 1, 2147483647, 2147483648, 4294967295]"),
    (
        "texts",
        "[\"\", \"a\", \"aa\", \"e\u{301}\", \"\\u{7f}\", \"\u{80}\", \"\u{e9}\", \"\u{ffff}\", \"\u{1f600}\", \"\u{10ffff}\"]",
    ),
    (
        "byte_strings",
        "[[], [0], [0, 255], [127], [128], [255], [255, 0]]",
    ),
    ("ones", "[One.Only, One.Only]"),
    ("twos", "[Two.Second, Two.First, Two.First]"),
    (
        "dirs",
        "[Dir.North, Dir.East, Dir.South, Dir.West, Dir.West]",
    ),
    (
        "std_levels",
        "[Level.Must, Level.MustNot, Level.Should, Level.May]",
    ),
    ("scores", "[1, 2, 3]"),
    (
        "enum_then_byte",
        "[(Dir.North, 2), (Dir.North, 9), (Dir.East, 0), (Dir.East, 1)]",
    ),
    (
        "tuple_text_int",
        r#"[("", 0), ("a", -3), ("a", 3), ("b", 1), ("b", 2)]"#,
    ),
    (
        "tuple3",
        r#"[(false, (200, (0, "z"))), (true, (0, (9, "q"))), (true, (1, (5, "x"))), (true, (1, (4294967295, "a")))]"#,
    ),
    (
        "padded",
        "[(1, -9, true, 0), (1, 9, false, 3), (2, 5, false, 2), (2, 5, true, 1)]",
    ),
    ("constant_key", "[5, 3, 9, 1, 7, 2, 8]"),
    (
        "tuple_first_equal",
        r#"[(0, "m"), (1, "z"), (1, "a"), (1, "b")]"#,
    ),
    (
        "box_sort",
        r#"(["[3]", "[1, 2]", "[]"], ["[1, 2, 99]", "[]", "[3]"])"#,
    ),
    ("bytes_values", "([[98], [97]], [[122], [98]])"),
    (
        "mutate_after",
        r#"(["b", "a", "c", "e"], ["q", "b", "c", "d"])"#,
    ),
    ("reassign", "[1, 2, 3, 0]"),
    ("nested_lists", r#"([["b", "a"], ["d", "c"]], ["c", "d"])"#),
    ("by_param", "([1, 2], [2, 1])"),
    ("temp_sort", "[7, 8, 9]"),
    ("captured", "[1, 2, 3]"),
    ("captured_key", "[0, 2, 1]"),
    ("match_bind", "[2, 3, 4]"),
    ("for_over_sorted", "[10, 20, 30]"),
    ("text_key_tuple", "[3, 1, 4, 2]"),
    ("keys_sorted", r#"["a", "b"]"#),
];

/// The fns of `program` without parameters, in declaration order.
fn printed_fns(program: &Program) -> Vec<String> {
    program
        .decls
        .iter()
        .filter_map(|decl| match decl {
            Decl::Fn(f) if f.params.is_empty() => Some(f.qualified_name()),
            _ => None,
        })
        .collect()
}

/// Each printed fn of `file` with its debug text by the evaluator.
fn evaluated(file: &Path) -> Vec<(String, String)> {
    let loaded = ignite::load(file, None).unwrap_or_else(|e| panic!("{e}"));
    let modules = loaded.modules();
    let mut evaluator = Evaluator::new(&modules, Fuel::with_units(10 * Fuel::DEFAULT_LIMIT));
    let module = loaded.types().module.clone();
    printed_fns(loaded.program())
        .into_iter()
        .map(|name| {
            let text =
                match evaluator.call(&QualName::new(module.clone(), name.clone()), Vec::new()) {
                    Ok(value) => value.debug_text(),
                    Err(e) => format!("evaluator error: {e:?}"),
                };
            (name, text)
        })
        .collect()
}

/// [`EDGES`] built for `target`, once per test binary.
fn edgeprint(target: Target) -> &'static Path {
    static BUILT: [OnceLock<PathBuf>; 4] = [const { OnceLock::new() }; 4];
    BUILT[target.index()].get_or_init(|| {
        todo::build(
            &source("sort_edges", EDGES),
            "list-sort-adversary-edges",
            target,
            "edgeprint",
        )
    })
}

/// The lines of `consumer` that differ from [`EDGES_EXPECTED`].
fn differences(consumer: &str, lines: &[(String, String)]) -> Vec<String> {
    let names: Vec<&str> = lines.iter().map(|(name, _)| name.as_str()).collect();
    let expected: Vec<&str> = EDGES_EXPECTED.iter().map(|(name, _)| *name).collect();
    assert_eq!(names, expected, "{consumer}: the printed fns");
    lines
        .iter()
        .zip(EDGES_EXPECTED)
        .filter(|((_, line), (_, want))| line != want)
        .map(|((name, line), (_, want))| format!("{consumer} {name}: {line}\n  expected {want}"))
        .collect()
}

/// The edges of every type with an order, the lengths around every width
/// of the runtime's merge sort (to 1,025), stability with 1 to 64 groups at
/// lengths one above a power of two, and the receiver left as it was in
/// every place a list can be held: the evaluator and both targets print
/// the text worked out by hand.
#[test]
fn edges_print_the_same_on_the_evaluator_and_both_targets() {
    let file = source("sort_edges", EDGES);
    let mut failures = differences("evaluator", &evaluated(&file));
    for target in [Target::Rust, Target::Llvm] {
        let (status, stdout, stderr) = run(edgeprint(target), &[]);
        assert_eq!(status, Some(0), "{target:?}: {stdout}\n{stderr}");
        let loaded = ignite::load(&file, None).unwrap_or_else(|e| panic!("{e}"));
        let lines: Vec<(String, String)> = printed_fns(loaded.program())
            .into_iter()
            .zip(stdout.lines().map(str::to_string))
            .collect();
        failures.extend(differences(&format!("{target:?}"), &lines));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// ---- diagnostics ---------------------------------------------------------------

/// `ignite check --json` of `program`: the code and message of each
/// diagnostic.
fn diagnostics(name: &str, program: &str) -> Vec<(String, String)> {
    let file = source(name, program);
    let output = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("check")
        .arg(&file)
        .arg("--json")
        .output()
        .expect("run ignite check");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let json = ignite_eval::Json::parse(&stdout).unwrap_or_else(|e| panic!("{e:?}: {stdout}"));
    let ignite_eval::Json::Object(top) = json else {
        panic!("not an object: {stdout}");
    };
    let Some(ignite_eval::Json::Array(items)) = top.get("diagnostics") else {
        panic!("no diagnostics array: {stdout}");
    };
    items
        .iter()
        .map(|item| {
            let ignite_eval::Json::Object(fields) = item else {
                panic!("a diagnostic that is no object: {stdout}");
            };
            let text = |key: &str| match fields.get(key) {
                Some(ignite_eval::Json::Str(text)) => text.clone(),
                other => panic!("{key}: {other:?}"),
            };
            (text("code"), text("message"))
        })
        .collect()
}

/// One fn per type without an order; each must be one E045.
const UNORDERED: &str = r#"package unordered

type Point {
  x: Int
}

enum Shape { Dot, Circle(Int) }

fn records(xs: List<Point>) -> List<Point> = xs.sort()

fn payload_enums(xs: List<Shape>) -> List<Shape> = xs.sort()

fn options(xs: List<Option<Int>>) -> List<Option<Int>> = xs.sort()

fn results(xs: List<Result<Int, Text>>) -> List<Result<Int, Text>> = xs.sort()

fn maps(xs: List<Map<Int, Int>>) -> List<Map<Int, Int>> = xs.sort()

fn lists(xs: List<List<Int>>) -> List<List<Int>> = xs.sort()

fn text_lists(xs: List<List<Text>>) -> List<List<Text>> = xs.sort()

fn fns(xs: List<fn(Int) -> Int>) -> List<fn(Int) -> Int> = xs.sort()

fn units(xs: List<Unit>) -> List<Unit> = xs.sort()

fn generic<T>(xs: List<T>) -> List<T> = xs.sort()

fn stores(xs: List<Store<Int>>) -> List<Store<Int>> = xs.sort()

fn generic_key<T, K>(xs: List<T>, k: fn(T) -> K) -> List<T> = xs.sort_by(k)

fn option_key(xs: List<Int>) -> List<Int> = xs.sort_by(|x| Some(x))

fn list_key(xs: List<Int>) -> List<Int> = xs.sort_by(|x| [x])

fn payload_key(xs: List<Int>) -> List<Int> = xs.sort_by(|x| Circle(x))

fn tuple_key(xs: List<Int>) -> List<Int> = xs.sort_by(|x| (x, Some(x)))
"#;

/// The element or key type each fn of [`UNORDERED`] names, in order.
const UNORDERED_TYPES: &[(&str, &str)] = &[
    ("sort", "`Point`"),
    ("sort", "`Shape`"),
    ("sort", "`Option<Int>`"),
    ("sort", "`Result<Int, Text>`"),
    ("sort", "`Map<Int, Int>`"),
    ("sort", "`List<Int>`"),
    ("sort", "`List<Text>`"),
    ("sort", "`fn(Int) -> Int`"),
    ("sort", "`Unit`"),
    ("sort", "`T`"),
    ("sort", "`Store<Int>`"),
    ("sort_by", "`K`"),
    ("sort_by", "`Option<Int>`"),
    ("sort_by", "`List<Int>`"),
    ("sort_by", "`Shape`"),
    ("sort_by", "`(Int, Option<Int>)`"),
];

/// `docs/slice.md` NN.1: E045 refuses `sort` on a list of any type without
/// an order, naming the element type and `sort_by`, and `sort_by` with a
/// key of any such type, naming the key's type; a generic `T` (and a
/// generic key `K`) has no order. One diagnostic per fn, in order.
#[test]
fn every_type_without_an_order_is_one_e045() {
    let found = diagnostics("unordered", UNORDERED);
    assert_eq!(found.len(), UNORDERED_TYPES.len(), "{found:#?}");
    for ((code, message), (call, ty)) in found.iter().zip(UNORDERED_TYPES) {
        assert_eq!(code, "E045", "{message}");
        assert!(
            message.contains(&format!("`{call}` needs")) && message.contains(ty),
            "{message} does not name `{call}` and {ty}"
        );
        if *call == "sort" {
            assert!(message.contains("sort_by"), "{message}");
        }
    }
}

/// A key fn's effects reach a caller whose effects are inferred, named or
/// closure, as `map`'s do.
const INFERRED: &str = r#"package inferred

fn stamp(id: Int) -> Int requires [clock.read] {
  let now = Clock.monotonic()
  return id
}

fn inferred_named(ids: List<Int>) -> List<Int> = ids.sort_by(stamp)

fn inferred_closure(ids: List<Int>) -> List<Int> = ids.sort_by(|i| stamp(i))

fn caller_named(ids: List<Int>) -> List<Int> requires [stdout.write] = inferred_named(ids)

fn caller_closure(ids: List<Int>) -> List<Int> requires [stdout.write] = inferred_closure(ids)
"#;

/// An effect on an unannotated closure parameter, as for `map`.
const ON_PARAMETER: &str = r#"package on_parameter

fn untyped(stores: List<Store<Text>>) -> List<Store<Text>> requires [stdout.write] = stores.sort_by(|s| s.get(1))
"#;

/// `docs/slice.md` NN.1: `f` may perform effects exactly where `map`'s may.
/// A `sort_by` with an effectful key fn, named or a closure, in a fn
/// without `requires` makes that fn effectful: a caller whose `requires`
/// lacks the effect is E016. A closure whose parameter's type is inferred
/// from the list gets the E017 of an effect on its parameter.
#[test]
fn a_key_fns_effects_are_charged_as_maps_are() {
    let found = diagnostics("inferred", INFERRED);
    assert_eq!(
        found,
        [
            (
                "E016".to_string(),
                "missing effect `clock.read` for `inferred_named`; add it to the `requires` of \
                 `caller_named`"
                    .to_string()
            ),
            (
                "E016".to_string(),
                "missing effect `clock.read` for `inferred_closure`; add it to the `requires` of \
                 `caller_closure`"
                    .to_string()
            ),
        ]
    );
    let found = diagnostics("on_parameter", ON_PARAMETER);
    assert_eq!(
        found,
        [(
            "E017".to_string(),
            "effectful closure passed to `sort_by` needs `s.read` on its parameter `s`, the \
             list's element; `sort_by` passes on only its caller's effects"
                .to_string()
        )]
    );
}

// ---- key fns that fail or print -------------------------------------------------

/// Key fns that fail, as fns without parameters for the evaluator and as
/// modes of `main` for the targets; `effect` prints each key it computes.
const KEYS: &str = r#"package sort_keys

type Runner {
  command_line: CommandLine
  executable: Text
}

fn show(text: Text) -> Result<Unit, IoError> requires [stdout.write] {
  var bytes = text.to_bytes()
  bytes.push(10)
  return Stdout.write_all(bytes)
}

fn trace(x: Int) -> Int requires [stdout.write] {
  let r = show("key {x}")
  x
}

fn div_case() -> List<Int> = [1, 0].sort_by(|x| 10 / x)

fn index_case() -> List<Int> {
  let ws = [1, 2]
  [0, 5].sort_by(|i| ws[i])
}

fn overflow_case() -> List<Int> = [1, 9223372036854775807].sort_by(|x| x + 1)

fn Runner.main(self: Runner) -> Result<Unit, IoError> {
  let mode = self.command_line.arguments()[0]
  if mode == "effect" {
    show([3, 1, 2].sort_by(trace).debug())?
    show([3, 1, 2].sort_by(|x| trace(x)).debug())?
    let none: List<Int> = []
    show(none.sort_by(trace).debug())?
  } else if mode == "div_case" {
    show(div_case().debug())?
  } else if mode == "index_case" {
    show(index_case().debug())?
  } else if mode == "overflow_case" {
    show(overflow_case().debug())?
  }
  return Ok(Unit)
}

system SortKeys {
  resource command_line: CommandLine
  component runner: Runner { command_line = command_line, executable = "sortkeys" }
}
"#;

/// [`KEYS`] built for `target`, once per test binary.
fn sortkeys(target: Target) -> &'static Path {
    static BUILT: [OnceLock<PathBuf>; 4] = [const { OnceLock::new() }; 4];
    BUILT[target.index()].get_or_init(|| {
        todo::build(
            &source("sort_keys", KEYS),
            "list-sort-adversary-keys",
            target,
            "sortkeys",
        )
    })
}

/// The report of a failure on stderr: the line holding ` failed at `,
/// its `error: ` prefix removed.
fn report_of(stderr: &str) -> Option<String> {
    stderr
        .lines()
        .find(|line| line.contains(" failed at "))
        .map(|line| line.strip_prefix("error: ").unwrap_or(line).to_string())
}

/// A key fn that fails (division by zero, an index out of range, an
/// overflow) fails the sort with the same report on the evaluator and both
/// targets, exit 101 on the targets.
#[test]
fn a_failing_key_fn_fails_with_one_report_everywhere() {
    let file = source("sort_keys", KEYS);
    let loaded = ignite::load(&file, None).unwrap_or_else(|e| panic!("{e}"));
    let modules = loaded.modules();
    let mut evaluator = Evaluator::new(&modules, Fuel::default());
    let module = loaded.types().module.clone();
    let mut failures = Vec::new();
    for case in ["div_case", "index_case", "overflow_case"] {
        let evaluated =
            match evaluator.call(&QualName::new(module.clone(), case.to_string()), Vec::new()) {
                Err(EvalError::Failed { report }) => report,
                other => format!("not a failure: {other:?}"),
            };
        for target in [Target::Rust, Target::Llvm] {
            let (status, stdout, stderr) = run(sortkeys(target), &[case]);
            let report = report_of(&stderr).unwrap_or_default();
            if status != Some(101) || report != evaluated {
                failures.push(format!(
                    "{case} {target:?}: exit {status:?}, report {report:?}, stdout {stdout:?}; \
                     the evaluator's {evaluated:?}"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// `docs/slice.md` NN.1: `f` is called once per element, in the order of
/// the list; never on an empty list. Both targets print the same keys.
#[test]
fn an_effectful_key_fn_runs_once_per_element_in_order_on_both_targets() {
    let expected = "key 3\nkey 1\nkey 2\n[1, 2, 3]\nkey 3\nkey 1\nkey 2\n[1, 2, 3]\n[]\n";
    for target in [Target::Rust, Target::Llvm] {
        let (status, stdout, stderr) = run(sortkeys(target), &["effect"]);
        assert_eq!(
            (status, stdout.as_str()),
            (Some(0), expected),
            "{target:?}: {stderr}"
        );
    }
}

// ---- places and key fns on the evaluator -----------------------------------------

/// Receivers in module constants, record fields and enum payloads; key fns
/// from a record field, a constant of fn type, a named fn and a generic fn.
const PLACES: &str = r#"package sort_places

const BASE: List<Int> = [5, 1, 4]

const NAMES: List<Text> = ["b", "c", "a"]

const NEG: fn(Int) -> Int = |x: Int| 0 - x

type Keyed {
  key: fn(Int) -> Int
}

type Shelf {
  id: Int
  words: List<Text>
  rows: List<Int>
}

enum Holder {
  Empty
  Many(List<Text>)
}

fn neg(x: Int) -> Int = 0 - x

fn sort_generic<T>(xs: List<T>, k: fn(T) -> Int) -> List<T> = xs.sort_by(k)

fn sort_generic_tuple<T>(xs: List<(Int, T)>) -> List<(Int, T)> = xs.sort_by(|p| p.0)

fn holder_sorted(h: Holder) -> List<Text> = match h {
  Many(items) => items.sort()
  Empty => []
}

test "constants sort and stay" {
  assert BASE.sort() == [1, 4, 5]
  assert BASE.sort_by(|x| 0 - x) == [5, 4, 1]
  assert BASE == [5, 1, 4]
  assert NAMES.sort() == ["a", "b", "c"]
  assert NAMES == ["b", "c", "a"]
  var c = BASE.sort()
  c.push(0)
  assert BASE == [5, 1, 4]
}

test "fields and payloads sort" {
  var s = Shelf { id = 1, words = ["q", "a", "m"], rows = [3, 2, 1] }
  let t = s
  let w = s.words.sort()
  let r = s.rows.sort_by(|x| x)
  s.words.push("z")
  assert w == ["a", "m", "q"]
  assert r == [1, 2, 3]
  assert t.words == ["q", "a", "m"]
  let u = Shelf { id = 2, words = t.words.sort(), rows = t.rows.sort() }
  var v = u
  v.words[0] = "first"
  assert u.words == ["a", "m", "q"]
  assert holder_sorted(Many(["y", "x"])) == ["x", "y"]
  let by_id = [s, t, u].sort_by(|x| 0 - x.id)
  assert by_id.map(|x| x.id) == [2, 1, 1]
  assert by_id[1].words == ["q", "a", "m", "z"]
}

test "key fns of every kind" {
  let k = Keyed { key = |x: Int| x % 3 }
  assert [1, 2, 3, 4, 5, 6].sort_by(k.key) == [3, 6, 1, 4, 2, 5]
  assert [1, 3, 2].sort_by(NEG) == [3, 2, 1]
  assert [1, 3, 2].sort_by(neg) == [3, 2, 1]
  assert sort_generic(["bb", "a", "ccc"], |t: Text| t.len()) == ["a", "bb", "ccc"]
  assert sort_generic_tuple([(2, "x"), (1, "y"), (2, "a")]) == [(1, "y"), (2, "x"), (2, "a")]
  let lists: List<List<Int>> = [[3, 1], [2]]
  assert lists.sort_by(|l| l.len()) == [[2], [3, 1]]
  assert lists.sort_by(|l| l.sort()[0]) == [[3, 1], [2]]
}
"#;

/// The tests of [`PLACES`] pass in the evaluator (both targets pass them
/// under `ignite test --target all`).
#[test]
fn places_and_key_fns_pass_in_the_evaluator() {
    let file = source("sort_places", PLACES);
    let loaded = ignite::load(&file, None).unwrap_or_else(|e| panic!("{e}"));
    let modules = loaded.modules();
    let mut evaluator = Evaluator::new(&modules, Fuel::default());
    let module = loaded.types().module.clone();
    let failures: Vec<String> = [
        "constants sort and stay",
        "fields and payloads sort",
        "key fns of every kind",
    ]
    .iter()
    .filter_map(|name| {
        evaluator
            .run_test(&module, name)
            .err()
            .map(|e| format!("{name}: {e:?}"))
    })
    .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// ---- copies on the Rust target --------------------------------------------------

/// `adv-alloc FORM N`: builds `N` distinct non-empty texts and, for `sort`
/// and `by`, sorts them with `sort()` or `sort_by(|t| t.len())`.
const ALLOC: &str = r#"package adv_alloc

type Cost {
  command_line: CommandLine
  executable: Text
}

fn number(t: Text) -> Int = match Int.parse(t) {
  Some(n) => n
  None => 0
}

fn texts(n: Int) -> List<Text> {
  var xs: List<Text> = List.with_capacity(n)
  var i = 0
  while i < n {
    xs.push("text number {(i * 7919) % n}")
    i = i + 1
  }
  xs
}

fn ok(t: Text) -> Result<Unit, IoError> requires [stdout.write] {
  var bytes = t.to_bytes()
  bytes.push(10)
  return Stdout.write_all(bytes)
}

fn Cost.main(self: Cost) -> Result<Unit, IoError> {
  let arguments = self.command_line.arguments()
  let form = arguments[0]
  let n = number(arguments[1])
  let xs = texts(n)
  if form == "sort" {
    let ys = xs.sort()
    ok("{ys.len()} {ys[0].len()}")?
  } else if form == "by" {
    let ys = xs.sort_by(|t| t.len())
    ok("{ys.len()} {ys[0].len()}")?
  } else {
    ok("{xs.len()} {xs[0].len()}")?
  }
  return Ok(Unit)
}

system AdvAlloc {
  resource command_line: CommandLine
  component cost: Cost { command_line = command_line, executable = "adv-alloc" }
}
"#;

/// The allocations `binary form n` makes, by memcheck's `total heap usage`.
fn allocations(binary: &Path, form: &str, n: u64) -> u64 {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("list-sort-adversary-valgrind");
    std::fs::create_dir_all(&dir).expect("the log directory");
    let log = dir.join(format!("memcheck-{form}-{n}-{}.log", std::process::id()));
    let output = Command::new("valgrind")
        .arg("--tool=memcheck")
        .arg(format!("--log-file={}", log.display()))
        .arg(binary)
        .args([form, &n.to_string()])
        .current_dir(&dir)
        .output()
        .unwrap_or_else(|e| panic!("valgrind is needed to count and did not run: {e}"));
    assert!(output.status.success(), "{form} {n}: {:?}", output.status);
    let text = std::fs::read_to_string(&log).expect("the valgrind log");
    let _ = std::fs::remove_file(&log);
    text.lines()
        .find_map(|line| line.split_once("total heap usage: ").map(|(_, rest)| rest))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|count| count.replace(',', "").parse().ok())
        .unwrap_or_else(|| panic!("no total heap usage in the valgrind log:\n{text}"))
}

/// `docs/slice.md` NN.2, Rust target: "`sort_by` computes the keys, sorts
/// the positions stably by key and copies each element once". A `sort_by`
/// of 1,000 texts on the Rust target then allocates about one block per
/// element (its copy) and a few for the keys, the positions and the new
/// list, as `sort()` does (1,000 copies and its `Vec`). The count of the
/// same run varies by a few blocks (1,041 to 1,046 for `build`), so 50
/// blocks are allowed beyond the 1,000 copies; a second copy of each is
/// 1,000 more.
#[test]
fn rust_sort_by_copies_each_element_once_as_written_down() {
    let binary = todo::build(
        &source("adv_alloc", ALLOC),
        "list-sort-adversary-alloc",
        Target::Rust,
        "adv-alloc",
    );
    let n = 1000;
    let built = allocations(&binary, "build", n);
    let sorted = allocations(&binary, "sort", n);
    let by = allocations(&binary, "by", n);
    println!("Rust: build {built}, build and sort {sorted}, build and sort_by {by}");
    assert!(
        sorted.saturating_sub(built) <= n + 50,
        "sort: {} allocations for {n} texts",
        sorted.saturating_sub(built)
    );
    assert!(
        by.saturating_sub(built) <= n + 50,
        "sort_by: {} allocations for {n} texts, more than one copy of each",
        by.saturating_sub(built)
    );
}
