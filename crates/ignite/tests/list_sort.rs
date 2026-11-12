//! Story `list-sort` (`docs/slice.md`, "A List is sorted with sort and
//! sort_by"): `xs.sort()` and `xs.sort_by(f)` on the evaluator and both
//! targets.
//!
//! `fixtures/list_sort.ignite` sorts every type with an order (`Int`,
//! `Byte`, `U32`, `Bool`, `Text`, `Bytes`, a payload-free enum, tuples of
//! them) and sorts records by keys; each consumer prints the debug text of
//! every fn without parameters, and each line must be the one [`EXPECTED`]
//! pins. Its inline tests are units of the differential corpus.
//!
//! [`COST`] is one executable, `sort-cost FORM N`, that builds the list of
//! `N` down to 1 and, for the form `sort`, sorts it. Its instructions
//! (callgrind's `Collected` line) less those of the form `build` are the
//! sort's; the count at 100,000 over the count at 10,000 bounds the growth
//! ([`a_reversed_list_of_100000_ints_sorts_in_n_log_n_instructions`]).
//! Its allocations (memcheck's `total heap usage` line) less those of
//! `build` are the sort's on the LLVM target, written down in
//! `docs/slice.md` ([`the_llvm_sort_of_1000_ints_allocates_no_more_than_written_down`]).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use ignite_eval::{Evaluator, Fuel};
use ignite_syntax::ast::{Decl, Program};
use ignite_types::QualName;

use crate::common::Target;
use crate::todo;

/// Every printed fn of the fixture and its debug text, in declaration
/// order.
const EXPECTED: &[(&str, &str)] = &[
    (
        "ints",
        "[-9223372036854775808, -1, 0, 2, 2, 3, 9223372036854775807]",
    ),
    ("bytes_sorted", "[0, 65, 97, 122, 255]"),
    ("words", "[0, 7, 2147483648, 4000000000]"),
    ("bools", "[false, false, true, true]"),
    ("texts", r#"["", "Z", "a", "a ", "ab", "b", "z", "é"]"#),
    ("byte_strings", "[[], [0], [97], [97, 98], [255]]"),
    (
        "levels",
        "[Level.Low, Level.Low, Level.Middle, Level.High, Level.High]",
    ),
    (
        "pairs",
        r#"[(-1, "q"), (1, "a"), (1, "z"), (2, "a"), (2, "b")]"#,
    ),
    (
        "nested",
        "[(Level.Low, (false, [122])), (Level.Low, (true, [])), \
         (Level.High, (false, [97])), (Level.High, (false, [98]))]",
    ),
    ("empty_and_one", r#"([], ["only"])"#),
    (
        "entries",
        r#"[Entry { name: "ann", level: Level.Middle, score: 3 }, Entry { name: "bob", level: Level.Low, score: 5 }, Entry { name: "cid", level: Level.High, score: 3 }, Entry { name: "dee", level: Level.Low, score: 1 }, Entry { name: "eve", level: Level.Middle, score: 5 }, Entry { name: "fay", level: Level.High, score: 1 }]"#,
    ),
    ("by_score", r#"["dee", "fay", "ann", "cid", "bob", "eve"]"#),
    ("by_level", r#"["bob", "dee", "ann", "eve", "cid", "fay"]"#),
    (
        "by_level_then_score_down",
        r#"["bob", "dee", "eve", "ann", "cid", "fay"]"#,
    ),
    (
        "by_named_fn",
        r#"["dee", "fay", "ann", "cid", "bob", "eve"]"#,
    ),
    ("by_text_key", "[10, 2, 33, 4]"),
    ("receiver_unchanged", "([3, 1, 2, 0], [1, 2, 3])"),
    (
        "sort_by_receiver_unchanged",
        r#"(["bb", "a", "ccc"], ["ccc", "bb", "a"])"#,
    ),
    ("thousand", "(1000, 1, 1000, true)"),
    ("stable_long", "(300, true)"),
];

/// The printed fns that hold the stability of `sort_by`: equal keys keep
/// the order they had.
const STABLE: &[&str] = &[
    "by_score",
    "by_level",
    "by_level_then_score_down",
    "by_named_fn",
    "stable_long",
];

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/list_sort.ignite")
}

/// The fns of the fixture without parameters, in declaration order: the
/// ones the printer's `main` shows.
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

/// Each printed fn with its debug text by the evaluator.
fn evaluated() -> Vec<(String, String)> {
    let loaded = ignite::load(&fixture(), None).unwrap_or_else(|e| panic!("{e}"));
    let modules = loaded.modules();
    let mut evaluator = Evaluator::new(&modules, Fuel::with_units(10 * Fuel::DEFAULT_LIMIT));
    let module = loaded.types().module.clone();
    printed_fns(loaded.program())
        .into_iter()
        .map(|name| {
            let value = evaluator
                .call(&QualName::new(module.clone(), name.clone()), Vec::new())
                .unwrap_or_else(|e| panic!("{name}: {e:?}"));
            (name, value.debug_text())
        })
        .collect()
}

/// The fixture's printer built for `target`, once per test binary.
fn sortprint(target: Target) -> &'static Path {
    static BUILT: [OnceLock<PathBuf>; 4] = [const { OnceLock::new() }; 4];
    BUILT[target.index()].get_or_init(|| todo::build(&fixture(), "list-sort", target, "sortprint"))
}

/// Each printed fn with the line the compiled printer wrote for it.
fn printed(target: Target) -> Vec<(String, String)> {
    let output = Command::new(sortprint(target))
        .output()
        .unwrap_or_else(|e| panic!("run sortprint: {e}"));
    assert!(
        output.status.success(),
        "{target:?}: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("debug text is UTF-8");
    let loaded = ignite::load(&fixture(), None).unwrap_or_else(|e| panic!("{e}"));
    let names = printed_fns(loaded.program());
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        lines.len(),
        names.len(),
        "{target:?} printed {} lines for {} fns:\n{stdout}",
        lines.len(),
        names.len()
    );
    names
        .into_iter()
        .zip(lines)
        .map(|(name, line)| (name, line.to_string()))
        .collect()
}

/// The lines of `consumer` that differ from [`EXPECTED`] for the fns
/// `only` names (every fn when empty).
fn differences(consumer: &str, lines: &[(String, String)], only: &[&str]) -> Vec<String> {
    let names: Vec<&str> = lines.iter().map(|(name, _)| name.as_str()).collect();
    let expected: Vec<&str> = EXPECTED.iter().map(|(name, _)| *name).collect();
    assert_eq!(names, expected, "{consumer}: the fixture's printed fns");
    lines
        .iter()
        .zip(EXPECTED)
        .filter(|((name, _), _)| only.is_empty() || only.contains(&name.as_str()))
        .filter(|((_, line), (_, want))| line != want)
        .map(|((name, line), (_, want))| format!("{consumer} {name}: {line}\n  expected {want}"))
        .collect()
}

/// The differences of the evaluator and both targets from [`EXPECTED`].
fn every_consumer(only: &[&str]) -> Vec<String> {
    let mut failures = differences("evaluator", &evaluated(), only);
    for target in [Target::Rust, Target::Llvm] {
        failures.extend(differences(&format!("{target:?}"), &printed(target), only));
    }
    failures
}

/// Acceptance 1 and 5: `sort` orders every type with an order ascending,
/// `Text` and `Bytes` by bytes, a payload-free enum by declaration and a
/// tuple element by element; `sort_by` orders by its key; the evaluator,
/// the Rust target and the LLVM target print the same text for each.
#[test]
fn sort_and_sort_by_print_the_same_on_the_evaluator_and_both_targets() {
    let failures = every_consumer(&[]);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Acceptance 2: `sort_by` is stable on all three consumers: records with
/// equal keys, and 300 pairs whose key repeats every third, keep the order
/// they had.
#[test]
fn sort_by_keeps_equal_keys_in_their_original_order() {
    let failures = every_consumer(STABLE);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// `ignite check --json` of `program`: the code and message of each
/// diagnostic.
fn diagnostics(name: &str, program: &str) -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("list-sort-check")
        .join(name);
    std::fs::create_dir_all(&dir).expect("the source directory");
    let file = dir.join(format!("{name}.ignite"));
    std::fs::write(&file, program).expect("write the program");
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

/// Acceptance 1: `sort` on a list of a type without an order is refused
/// by `ignite check` with one diagnostic, and it names `sort_by`; so is
/// `sort_by` with a key without an order, naming the key's type.
#[test]
fn sort_without_an_order_is_one_diagnostic_that_names_sort_by() {
    let record = diagnostics(
        "record_sort",
        "package record_sort\n\ntype Point {\n  x: Int\n  y: Int\n}\n\n\
         fn sorted(ps: List<Point>) -> List<Point> = ps.sort()\n",
    );
    assert_eq!(record.len(), 1, "{record:?}");
    assert!(
        record[0].1.contains("sort_by") && record[0].1.contains("Point"),
        "{record:?}"
    );
    let option = diagnostics(
        "option_sort",
        "package option_sort\n\nfn sorted(xs: List<Option<Int>>) -> List<Option<Int>> = xs.sort()\n",
    );
    assert_eq!(option.len(), 1, "{option:?}");
    assert!(option[0].1.contains("sort_by"), "{option:?}");
    let key = diagnostics(
        "record_key",
        "package record_key\n\ntype Point {\n  x: Int\n  y: Int\n}\n\n\
         fn sorted(ps: List<Point>) -> List<Point> = ps.sort_by(|p| p)\n",
    );
    assert_eq!(key.len(), 1, "{key:?}");
    assert!(key[0].1.contains("Point"), "{key:?}");
    let fine = diagnostics(
        "ordered",
        "package ordered\n\ntype Point {\n  x: Int\n  y: Int\n}\n\n\
         fn sorted(ps: List<Point>) -> List<Point> = ps.sort_by(|p| (p.x, p.y))\n\n\
         fn plain(xs: List<(Text, Bytes, Bool)>) -> List<(Text, Bytes, Bool)> = xs.sort()\n",
    );
    assert!(fine.is_empty(), "{fine:?}");
}

/// `sort_by`'s key fn may perform effects exactly where `map`'s fn may
/// (`docs/slice.md` 20.4.3): an effect the caller's `requires` does not
/// cover, or one on the key fn's own parameter, is E017 naming `sort_by`;
/// one the `requires` covers is fine.
#[test]
fn sort_by_takes_the_effects_map_takes() {
    let found = diagnostics(
        "effects",
        "package effects\n\n\
         fn stamp(id: Int) -> Int requires [clock.read] {\n  let now = Clock.monotonic()\n  return id\n}\n\n\
         fn stamps(ids: List<Int>) -> List<Int> requires [stdout.write] = ids.sort_by(stamp)\n\n\
         fn covered(ids: List<Int>) -> List<Int> requires [clock.read] = ids.sort_by(stamp)\n\n\
         fn firsts(stores: List<Store<Text>>) -> List<Store<Text>> requires [stores.read] = \
         stores.sort_by(|s: Store<Text>| s.get(1))\n",
    );
    assert_eq!(
        found,
        [
            (
                "E017".to_string(),
                "effectful function `stamp` passed to `sort_by` needs `clock.read`, which the \
                 `requires` of `stamps` does not cover"
                    .to_string()
            ),
            (
                "E017".to_string(),
                "effectful closure passed to `sort_by` needs `s.read` on its parameter `s`, the \
                 list's element; `sort_by` passes on only its caller's effects"
                    .to_string()
            ),
        ]
    );
}

/// The counting executable.
const COST: &str = r#"package sort_cost

type Cost {
  command_line: CommandLine
  executable: Text
}

fn number(t: Text) -> Int = match Int.parse(t) {
  Some(n) => n
  None => 0
}

fn reversed(n: Int) -> List<Int> {
  var xs: List<Int> = List.with_capacity(n)
  var i = n
  while i > 0 {
    xs.push(i)
    i = i - 1
  }
  xs
}

fn Cost.main(self: Cost) -> Result<Unit, IoError> {
  let arguments = self.command_line.arguments()
  let n = number(arguments[1])
  let xs = reversed(n)
  if arguments[0] == "sort" {
    let ys = xs.sort()
    log("sorted {ys[0]} {ys[n - 1]}")
  } else {
    log("built {xs[n - 1]} {xs[0]}")
  }
  return Ok(Unit)
}

system SortCost {
  resource command_line: CommandLine
  component cost: Cost { command_line = command_line, executable = "sort-cost" }
}
"#;

/// [`COST`] built for `target`, once per test binary.
fn cost(target: Target) -> &'static Path {
    static BUILT: [OnceLock<PathBuf>; 4] = [const { OnceLock::new() }; 4];
    BUILT[target.index()].get_or_init(|| {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("list-sort-cost-source");
        std::fs::create_dir_all(&dir).expect("the source directory");
        let source = dir.join("sort_cost.ignite");
        std::fs::write(&source, COST).expect("write the source");
        todo::build(&source, "list-sort-cost", target, "sort-cost")
    })
}

/// `valgrind --tool=<tool> binary form n` run to its end; the number on
/// the first line of its log that holds `marker`.
fn valgrind_count(binary: &Path, tool: &str, marker: &str, form: &str, n: u64) -> u64 {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("list-sort-valgrind");
    std::fs::create_dir_all(&dir).expect("the log directory");
    let stem = format!("{tool}-{form}-{n}-{}", std::process::id());
    let log = dir.join(format!("{stem}.log"));
    let mut command = Command::new("valgrind");
    command
        .arg(format!("--tool={tool}"))
        .arg(format!("--log-file={}", log.display()));
    if tool == "callgrind" {
        command.arg(format!(
            "--callgrind-out-file={}",
            dir.join(format!("{stem}.out")).display()
        ));
    }
    let output = command
        .arg(binary)
        .args([form, &n.to_string()])
        .current_dir(&dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .unwrap_or_else(|e| panic!("valgrind is needed to count and did not run: {e}"));
    assert!(
        output.status.success(),
        "{form} {n}: {:?}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let text = std::fs::read_to_string(&log).expect("the valgrind log");
    let _ = std::fs::remove_file(&log);
    let _ = std::fs::remove_file(dir.join(format!("{stem}.out")));
    text.lines()
        .find_map(|line| line.split_once(marker).map(|(_, rest)| rest))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|count| count.replace(',', "").parse().ok())
        .unwrap_or_else(|| panic!("no `{marker}` in the valgrind log:\n{text}"))
}

/// The instructions `binary form n` executes, by callgrind.
fn instructions(binary: &Path, form: &str, n: u64) -> u64 {
    valgrind_count(binary, "callgrind", "Collected : ", form, n)
}

/// The allocations `binary form n` makes, by memcheck.
fn allocations(binary: &Path, form: &str, n: u64) -> u64 {
    valgrind_count(binary, "memcheck", "total heap usage: ", form, n)
}

/// The ratio of the sort's instructions at 100,000 elements to those at
/// 10,000 that O(n log n) stays under and O(n^2) does not: n log2 n gives
/// 10 x 16.6 / 13.3 = 12.5, n^2 gives 100.
const GROWTH: f64 = 20.0;

/// Acceptance 4: a list of 100,000 `Int` in reverse order sorts in
/// O(n log n) on both targets: the sort's instructions (callgrind, those
/// of building the list taken off) grow by at most [`GROWTH`] from 10,000
/// elements.
#[test]
fn a_reversed_list_of_100000_ints_sorts_in_n_log_n_instructions() {
    let mut failures = Vec::new();
    for target in [Target::Rust, Target::Llvm] {
        let binary = cost(target);
        let sort = |n: u64| {
            instructions(binary, "sort", n).saturating_sub(instructions(binary, "build", n))
        };
        let small = sort(10_000);
        let large = sort(100_000);
        let ratio = large as f64 / small.max(1) as f64;
        println!(
            "{target:?}: sort of 10,000 reversed Ints {small} instructions, of 100,000 {large}; \
             ratio {ratio:.2}"
        );
        if small == 0 || ratio > GROWTH {
            failures.push(format!(
                "{target:?}: {small} instructions at 10,000, {large} at 100,000: ratio {ratio:.2} \
                 over {GROWTH}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The allocations of a sort of 1,000 `Int` on the LLVM target, written
/// down in `docs/slice.md`: none. The positions and the merge's scratch
/// (16,000 bytes) and the new list (8,000 bytes) come from the task's
/// arena, whose first 64 KiB chunk holds them; the runtime's merge sort
/// allocates nothing of its own. Measured 32 `malloc` calls with the sort
/// and 32 without (memcheck), and 15 `mmap` calls each (`strace -c`).
const LLVM_SORT_1000: u64 = 0;

/// Acceptance 5: the LLVM target's allocation count for a sort of 1,000
/// elements (memcheck, those of building the list taken off) is no more
/// than [`LLVM_SORT_1000`].
#[test]
fn the_llvm_sort_of_1000_ints_allocates_no_more_than_written_down() {
    let binary = cost(Target::Llvm);
    let sorted = allocations(binary, "sort", 1000);
    let built = allocations(binary, "build", 1000);
    println!("Llvm: building 1,000 Ints {built} allocations, building and sorting {sorted}");
    assert!(
        sorted <= built + LLVM_SORT_1000,
        "the sort of 1,000 Ints allocates {} times, {LLVM_SORT_1000} written down",
        sorted.saturating_sub(built)
    );
}
