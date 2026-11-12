//! Story `std-mysql-client`, unit `mysql-cost`: a lookup that only reads
//! the elements of lists allocates nothing on the Rust target
//! (`docs/slice.md` 20.3.7). `fixtures/borrowed_reads.ignite` has one of
//! each read-only form: a tuple pattern in a `for`, an indexed element
//! passed to a borrowed parameter, matched, iterated, measured and
//! compared, an element of an element, and a `for` over a list of integers.
//! This test appends [`EXECUTABLES`] to it, which build a table of 50
//! elements and make 1,000 (`lookups-1000`) or 2,000 (`lookups-2000`)
//! lookups over it. The executables stay out of the fixture: the
//! differential corpus would count their `main`s as units the evaluator
//! cannot run (metric 1.6). The allocations the second run makes more than
//! the first are those of 1,000 lookups, counted as `reply_cost::` counts
//! them, with `valgrind --tool=memcheck --trace-malloc=yes`, a prerequisite
//! of this test as of that one.
//!
//! The LLVM target allocates from its task's arena, which `malloc` does
//! not see; the test runs its executables for the values only, which must
//! be the Rust target's.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::common::Target;
use crate::todo;

/// Lookups the second executable makes more than the first.
const EXTRA: f64 = 1000.0;

/// The two executables appended to the fixture.
const EXECUTABLES: &str = r#"
type Lookups {
  executable: Text
  count: Int
}

fn Lookups.main(self: Lookups) -> Result<Unit, IoError> {
  log("{self.count} lookups: {lookups(table(50), self.count)}")
  return Ok(Unit)
}

system BorrowedReads {
  component fewer: Lookups { executable = "lookups-1000", count = 1000 }
  component more: Lookups { executable = "lookups-2000", count = 2000 }
}
"#;

/// The two executables of the fixture built for `target`.
fn executables(target: Target) -> (PathBuf, PathBuf) {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/borrowed_reads.ignite");
    let text = std::fs::read_to_string(&fixture).expect("the fixture");
    let dir =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("borrowed-reads-source-{target:?}"));
    std::fs::create_dir_all(&dir).expect("the source directory");
    let source = dir.join("borrowed_reads.ignite");
    std::fs::write(&source, text + EXECUTABLES).expect("write the source");
    let out = format!("borrowed-reads-{target:?}");
    (
        todo::build(&source, &out, target, "lookups-1000"),
        todo::build(&source, &out, target, "lookups-2000"),
    )
}

/// The standard error of `binary` run to its end, which must succeed.
fn logged(mut command: Command) -> String {
    let output = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .expect("run the executable");
    let text = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(output.status.success(), "{:?}: {text}", output.status);
    text
}

/// The allocation calls (`malloc`, `calloc`, `memalign` and the like) of
/// `binary` run to its end under valgrind, and its log line.
fn allocations(binary: &Path) -> (u64, String) {
    let log = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "borrowed-reads-{}-{}.log",
        binary.file_name().and_then(|n| n.to_str()).unwrap_or("run"),
        std::process::id()
    ));
    let mut command = Command::new("valgrind");
    command
        .args(["--tool=memcheck", "--trace-malloc=yes"])
        .arg(format!("--log-file={}", log.display()))
        .arg(binary);
    let line = logged(command);
    let text = std::fs::read_to_string(&log).expect("the valgrind log");
    let count = text
        .lines()
        .filter_map(|line| {
            line.strip_prefix("--")
                .and_then(|rest| rest.split_once("-- "))
        })
        .filter(|(_, call)| {
            matches!(
                call.split('(').next().unwrap_or(""),
                "malloc" | "calloc" | "memalign" | "posix_memalign" | "aligned_alloc" | "valloc"
            )
        })
        .count() as u64;
    let _ = std::fs::remove_file(&log);
    (count, line.trim().to_string())
}

#[test]
fn a_lookup_that_only_reads_allocates_nothing_on_the_rust_target() {
    let status = Command::new("valgrind")
        .args(["--tool=memcheck", "--log-file=/dev/null", "true"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap_or_else(|e| panic!("valgrind is needed to count allocations and did not run: {e}"));
    assert!(status.success(), "valgrind did not run `true`: {status:?}");
    let (fewer, more) = executables(Target::Rust);
    let (first, first_line) = allocations(&fewer);
    let (second, second_line) = allocations(&more);
    let per_lookup = second.saturating_sub(first) as f64 / EXTRA;
    println!("Rust: {first} and {second} allocations, {per_lookup:.3} per lookup");
    assert!(
        per_lookup < 0.01,
        "a read-only lookup allocates {per_lookup:.3} times on the Rust target \
         ({first} allocations for 1,000 lookups, {second} for 2,000)"
    );
    let (fewer_llvm, more_llvm) = executables(Target::Llvm);
    assert_eq!(
        logged(Command::new(&fewer_llvm)).trim(),
        first_line,
        "LLVM, 1,000 lookups"
    );
    assert_eq!(
        logged(Command::new(&more_llvm)).trim(),
        second_line,
        "LLVM, 2,000 lookups"
    );
}

/// `l[0][j]` passed by reference whose index `j` replaces `l`. The outer
/// read is not lent (its index changes `l`), and the inner `l[0]` must not
/// be either: the index is evaluated while it would be borrowed. The Rust
/// target compiles it and reads `l[0]` before the index, giving 3, as at
/// `1e16dfba`. The evaluator reads the index first and gives 2, a
/// difference older than this unit and not pinned here.
const INDEX_REPLACES_THE_LIST: &str = r#"package index_replaces_the_list

enum V {
  N
  S(Text)
}

fn size(v: V) -> Int = match v {
  S(t) => t.len()
  N => 0
}

fn element_of_element() -> Int {
  var l = [[S("abc")], [N]]
  size(l[0][if true {
    l = [[S("zz")]]
    0
  } else {
    0
  }])
}

test "the element of an element is read before its index replaces the list" {
  assert element_of_element() == 3
}
"#;

#[test]
fn an_element_of_an_element_whose_index_replaces_the_list_compiles_on_the_rust_target() {
    let file =
        crate::borrowed_reads_adversary::source("index_replaces_the_list", INDEX_REPLACES_THE_LIST);
    if let Some(failure) = crate::borrowed_reads_adversary::target_failure(&file, "rust", 1) {
        panic!("{failure}");
    }
}

/// Unit `call-moves`: the executables appended to
/// `fixtures/call_moves.ignite`. Form 9 makes `count` updates
/// `r = set_in(r, "a", read_in(r, "b")?)` of a record whose `rows` hold
/// `rows` texts; form 7 makes `count` matches `match (a, b)` of two records
/// of `rows` fields each.
const CALL_EXECUTABLES: &str = r#"
type Calls {
  executable: Text
  form: Int
  rows: Int
  count: Int
}

fn updated(m: Int, n: Int) -> Int = match updates(m, n) {
  Ok(k) => k
  Err(_) => -1
}

fn Calls.main(self: Calls) -> Result<Unit, IoError> {
  if self.form == 9 {
    log("{self.count} updates of {self.rows} rows: {updated(self.rows, self.count)}")
  } else {
    log("{self.count} matches of {self.rows} fields: {compares(self.rows, self.count)}")
  }
  return Ok(Unit)
}

system CallMoves {
  component updates_small_fewer: Calls { executable = "updates-10-1000", form = 9, rows = 10, count = 1000 }
  component updates_small_more: Calls { executable = "updates-10-2000", form = 9, rows = 10, count = 2000 }
  component updates_large_fewer: Calls { executable = "updates-200-1000", form = 9, rows = 200, count = 1000 }
  component updates_large_more: Calls { executable = "updates-200-2000", form = 9, rows = 200, count = 2000 }
  component matches_small_fewer: Calls { executable = "matches-10-1000", form = 7, rows = 10, count = 1000 }
  component matches_small_more: Calls { executable = "matches-10-2000", form = 7, rows = 10, count = 2000 }
  component matches_large_fewer: Calls { executable = "matches-200-1000", form = 7, rows = 200, count = 1000 }
  component matches_large_more: Calls { executable = "matches-200-2000", form = 7, rows = 200, count = 2000 }
}
"#;

/// The executable `name` of `fixtures/call_moves.ignite` with
/// [`CALL_EXECUTABLES`] appended, built for `target`.
fn call_executable(target: Target, name: &str) -> PathBuf {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/call_moves.ignite");
    let text = std::fs::read_to_string(&fixture).expect("the fixture");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("call-moves-source-{target:?}"));
    std::fs::create_dir_all(&dir).expect("the source directory");
    let source = dir.join("call_moves.ignite");
    std::fs::write(&source, text + CALL_EXECUTABLES).expect("write the source");
    todo::build(&source, &format!("call-moves-{target:?}"), target, name)
}

/// The allocations one operation of `form` (`updates` or `matches`) makes
/// over `rows` elements on the Rust target, from the runs of 1,000 and
/// 2,000 operations, and the two log lines.
fn per_operation(form: &str, rows: u32) -> (f64, [String; 2]) {
    let fewer = call_executable(Target::Rust, &format!("{form}-{rows}-1000"));
    let more = call_executable(Target::Rust, &format!("{form}-{rows}-2000"));
    let (first, first_line) = allocations(&fewer);
    let (second, second_line) = allocations(&more);
    let per = second.saturating_sub(first) as f64 / EXTRA;
    println!("Rust {form}, {rows} rows: {first} and {second} allocations, {per:.3} per operation");
    (per, [first_line, second_line])
}

/// Form 9 and form 7 of the reporter's program: an update of a record that
/// a later argument reads, and a match on a tuple of two records, allocate
/// as many times over records of 200 elements as over records of 10 on the
/// Rust target (before the unit, one copy of the record or of both per
/// operation: 190 and 380 allocations more). The LLVM target gives the
/// same values.
#[test]
fn an_update_read_by_a_later_argument_and_a_tuple_match_do_not_copy_the_record() {
    let status = Command::new("valgrind")
        .args(["--tool=memcheck", "--log-file=/dev/null", "true"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap_or_else(|e| panic!("valgrind is needed to count allocations and did not run: {e}"));
    assert!(status.success(), "valgrind did not run `true`: {status:?}");
    let mut failures = Vec::new();
    for form in ["updates", "matches"] {
        let (small, small_lines) = per_operation(form, 10);
        let (large, large_lines) = per_operation(form, 200);
        if large - small >= 0.5 {
            failures.push(format!(
                "{form}: {small:.3} allocations per operation over 10 elements, \
                 {large:.3} over 200"
            ));
        }
        for (rows, lines) in [(10, small_lines), (200, large_lines)] {
            for (count, line) in [1000, 2000].iter().zip(lines) {
                let llvm = call_executable(Target::Llvm, &format!("{form}-{rows}-{count}"));
                let text = logged(Command::new(&llvm));
                if text.trim() != line {
                    failures.push(format!("LLVM {form}-{rows}-{count}: {text} against {line}"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Every inline test of `fixtures/call_moves.ignite`, the forms and one
/// program per guard, passes on the evaluator and both targets.
#[test]
fn each_guard_of_a_call_move_gives_the_evaluator_value_on_both_targets() {
    crate::borrowed_reads_adversary::agree(
        "call_moves",
        include_str!("fixtures/call_moves.ignite"),
        6,
    );
}
