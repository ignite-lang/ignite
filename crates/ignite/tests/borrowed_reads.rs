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

/// Unit `call-moves`, correction round: the guards of a late move and of
/// a lent tuple that no other program decides, and the arms that take
/// what a lent tuple binds. Each test names what it decides:
///
/// - `variant_late`: a late move only in a fn call. A variant payload is
///   a move position too, but only a fn call binds its later arguments
///   first; marked late in `Two(r, r.count + 1)`, the evaluator, whose
///   `late_args` wraps fn calls only, would take the inner `r` once the
///   enclosing `width(..)` call's arguments ran, out of the outer `r`
///   ("`()` has no field `rows`").
/// - `int_and_text`: a tuple with a text pattern is not lent. The `Int`
///   item would be a value (`Copy`), which the guard test of a text
///   pattern reads as a reference (`*__p1`; rustc E0614).
/// - `binding_moved`, `binding_moved_late`, `whole_moved`,
///   `whole_returned`, `binding_overwritten`, `list_binding_overwritten`:
///   a binding of an item or of the whole tuple moved into a callee that
///   changes it, late or not, or returned and replaced; the place in the
///   tuple keeps its value on both targets. The Rust target copies a lent
///   binding where it moves; the LLVM target does not lend a tuple whose
///   arm moves a binding (lent, the callee's `o.rows[0] = ..` wrote the
///   place's own buffer: 4143 and 4104).
/// - `apply_pair`, `apply_local`: an item of a fn type; `first_of`: a type
///   parameter. Neither is lent: each may hold a handle.
/// - `read_after`, `pair_of_ends`, `closed_after`, `returned_binding`: a
///   channel end, or a record that holds one, matched in a tuple and used
///   after it, its binding returned from the `match` included. The LLVM
///   target lends such a tuple as any other (no handle exclusion: a tuple
///   literal is never an owned match, so neither form releases it).
const CALL_MOVE_GUARDS: &str = r#"package call_move_guards_two

enum Val {
  Nil
  Num(Int)
  Txt(Text)
}

type Row {
  k: Text
  v: Val
}

type Ctx {
  label: Text
  rows: List<Row>
  count: Int
}

fn ctx(m: Int) -> Ctx {
  var rows: List<Row> = []
  var i = 0
  while i < m {
    rows.push(Row { k = "r{i}", v = Txt("t{i}") })
    i = i + 1
  }
  Ctx { label = "ctx", rows = rows, count = 0 }
}

fn size(v: Val) -> Int = match v {
  Nil => 0
  Num(n) => n
  Txt(t) => t.len()
}

enum Pack {
  Two(Ctx, Int)
}

fn width(p: Pack) -> Int = match p {
  Two(c, n) => c.rows.len() * 100 + c.count * 10 + n
}

fn variant_late() -> Int {
  let r = ctx(2)
  let n = width(if r.count == 0 {
    let r = ctx(3)
    Two(r, r.count + 1)
  } else {
    Two(ctx(0), 0)
  })
  n * 10 + r.rows.len()
}

fn int_and_text(n: Int, t: Text) -> Int = match (n, t) {
  (1, "x") => 10
  (k, "y") => k * 100
  _ => -1
}

fn int_and_text_local(m: Int) -> Int {
  let n = m
  let t = "y"
  match (n, t) {
    (1, "x") => 10
    (k, "y") => k * 100
    _ => -1
  }
}

fn bump(c: Ctx) -> Int {
  var o = c
  o.rows.push(Row { k = "new", v = Nil })
  o.label = "changed"
  o.rows.len()
}

fn binding_moved() -> Int {
  let r = ctx(2)
  let v = Num(1)
  let n = match (r, v) {
    (c, Num(k)) => bump(c) * 10 + k
    _ => 0
  }
  n * 100 + r.rows.len() * 10 + r.label.len()
}

fn recount(c: Ctx, n: Int) -> Int {
  var o = c
  o.count = n
  o.rows.push(Row { k = "new", v = Nil })
  o.rows.len() * 10 + o.count
}

fn binding_moved_late() -> Int {
  let r = ctx(2)
  let v = Num(1)
  let n = match (r, v) {
    (c, Num(k)) => recount(c, c.rows.len() + k)
    _ => 0
  }
  n * 10 + r.rows.len()
}

fn bump_pair(p: (Ctx, Val)) -> Int {
  var q = p
  let before = q.0.rows.len() * 10 + size(q.1)
  q = (ctx(5), Nil)
  before + q.0.rows.len()
}

fn whole_moved() -> Int {
  let r = ctx(2)
  let v = Num(1)
  let n = match (r, v) {
    (_, Nil) => 0
    pair => bump_pair(pair)
  }
  n * 100 + r.rows.len() * 10 + r.label.len()
}

fn whole_returned() -> Int {
  let r = ctx(2)
  let v = Num(1)
  var p = match (r, v) {
    (_, Nil) => (ctx(0), Nil)
    pair => pair
  }
  let before = p.0.rows.len()
  p = (ctx(4), Nil)
  before * 1000 + p.0.rows.len() * 100 + r.rows.len() * 10 + r.rows[1].k.len()
}

fn overwrite(c: Ctx) -> Int {
  var o = c
  o.rows[0] = Row { k = "zzzz", v = Nil }
  o.label = "changed"
  o.rows[0].k.len()
}

fn binding_overwritten() -> Int {
  let r = ctx(2)
  let v = Num(1)
  let n = match (r, v) {
    (c, Num(k)) => overwrite(c) * 10 + k
    _ => 0
  }
  n * 100 + r.rows[0].k.len() * 10 + r.label.len()
}

fn overwrite_list(l: List<Text>) -> Int {
  var o = l
  o[0] = "zzzz"
  o[0].len()
}

fn list_binding_overwritten() -> Int {
  let l = ["ab", "c"]
  let v = Num(1)
  let n = match (l, v) {
    (rows, Num(k)) => overwrite_list(rows) * 10 + k
    _ => 0
  }
  n * 100 + l[0].len()
}

fn apply_pair(f: fn(Int) -> Int, v: Val) -> Int = match (f, v) {
  (g, Num(n)) => g(n)
  _ => -1
}

fn twice(f: fn(Int) -> Int, n: Int) -> Int = f(f(n))

fn apply_local(m: Int) -> Int {
  let f = |x: Int| x + m
  let v = Num(2)
  match (f, v) {
    (g, Num(n)) => twice(g, n) + f(1)
    _ => -1
  }
}

fn first_of<T>(x: T, v: Val, keep: fn(T) -> Int) -> Int = match (x, v) {
  (y, Num(n)) => keep(y) + n
  _ => -1
}

type Link {
  tx: Sender<Int>
  n: Int
}

fn open_link() -> (Link, Receiver<Int>) {
  let (tx, rx) = Channel.bounded(4)
  (Link { tx = tx, n = 2 }, rx)
}

fn read_after() -> Int {
  let ends: (Link, Receiver<Int>) = open_link()
  let v = Num(3)
  let got = match (ends.0, v) {
    (l, Num(k)) => l.n + k
    _ => 0
  }
  let sent = ends.0.tx.try_send(got)
  let back = match ends.1.receive_within(0 ms) {
    Some(k) => k
    None => -1
  }
  if sent {
    back * 10 + 1
  } else {
    back * 10
  }
}

fn look(link: Link, v: Val) -> Int {
  let got = match (link, v) {
    (l, Num(k)) => l.n + k
    _ => 0
  }
  got + link.n
}

fn looked() -> Receiver<Int> {
  let (link, rx) = open_link()
  let got = look(link, Num(3))
  rx
}

fn closed_after() -> Bool {
  let rx = looked()
  rx.closed()
}

fn pair_of_ends() -> Int {
  let (tx, rx) = Channel.bounded(4)
  let v = Num(3)
  let got = match (tx, v) {
    (t, Num(k)) => k
    _ => 0
  }
  let sent = tx.try_send(got)
  match rx.receive_within(0 ms) {
    Some(k) => k
    None => -1
  }
}

fn returned_binding() -> Int {
  let (link, rx) = open_link()
  let (spare, rx2) = open_link()
  let v = Num(3)
  let got = match (link, v) {
    (l, Num(_)) => l
    _ => spare
  }
  let first = got.n
  let sent = link.tx.try_send(7)
  let back = match rx.receive_within(0 ms) {
    Some(k) => k
    None => -1
  }
  if sent {
    first * 100 + back * 10 + 1
  } else {
    first * 100 + back * 10
  }
}

test "a late move happens only in a fn call" {
  assert variant_late() == 3012
}

test "an int and a text matched with a text pattern" {
  assert int_and_text(1, "x") == 10
  assert int_and_text(3, "y") == 300
  assert int_and_text(3, "z") == -1
  assert int_and_text_local(4) == 400
}

test "a binding of a tuple item moved into a change" {
  assert binding_moved() == 3123
}

test "a binding of a tuple item moved late into a change" {
  assert binding_moved_late() == 332
}

test "the whole tuple moved into a change" {
  assert whole_moved() == 2623
}

test "the whole tuple returned and replaced" {
  assert whole_returned() == 2422
}

test "a binding of a tuple item moved into a callee that overwrites an element" {
  assert binding_overwritten() == 4123
  assert list_binding_overwritten() == 4102
}

test "a fn value matched in a tuple" {
  assert apply_pair(|x| x * 2, Num(4)) == 8
  assert apply_local(3) == 12
}

test "a generic value matched in a tuple" {
  assert first_of("abc", Num(1), |t| t.len()) == 4
}

test "a record holding a sender read after a tuple match" {
  assert read_after() == 51
}

test "a sender read after a tuple match" {
  assert pair_of_ends() == 3
}

test "a borrowed record holding a sender matched in a tuple leaves no owner behind" {
  assert closed_after()
}

test "a binding of a borrowed record holding a sender returned from a tuple match" {
  assert returned_binding() == 271
}
"#;

#[test]
fn each_guard_of_the_correction_round_gives_the_evaluator_value_on_both_targets() {
    crate::borrowed_reads_adversary::agree("call_move_guards_two", CALL_MOVE_GUARDS, 13);
}

/// A record that holds the only sender of a channel, moved into a `match`
/// on a tuple at its last use (directly, or as a parameter the callee
/// consumes), is dropped with the tuple at the end of the `match`: the
/// channel is closed right after it. The evaluator, which takes a moved
/// variable under a test, says so. Lent as `match (&link, &v)` the Rust
/// target kept the record to the end of its scope (`rx.closed()` false,
/// 50); a tuple that may hold a handle is not lent
/// ([`ignite_types::TypedProgram::holds_handles`]). The LLVM target never
/// releases a handle-holding place moved into a tuple scrutinee, at
/// 1dfc2937 as now: it fails both tests with 50, which this test holds.
const HANDLE_TUPLE: &str = r#"package handle_tuple

enum Val {
  Nil
  Num(Int)
}

type Link {
  tx: Sender<Int>
  n: Int
}

fn open_link() -> (Link, Receiver<Int>) {
  let (tx, rx) = Channel.bounded(4)
  (Link { tx = tx, n = 2 }, rx)
}

fn in_place() -> Int {
  let (link, rx) = open_link()
  let v = Num(3)
  let got = match (link, v) {
    (l, Num(k)) => l.n + k
    _ => 0
  }
  if rx.closed() {
    got * 10 + 1
  } else {
    got * 10
  }
}

fn look(link: Link, v: Val) -> Int = match (link, v) {
  (l, Num(k)) => l.n + k
  _ => 0
}

fn in_callee() -> Int {
  let (link, rx) = open_link()
  let got = look(link, Num(3))
  if rx.closed() {
    got * 10 + 1
  } else {
    got * 10
  }
}

test "a record holding a sender moved into a tuple match" {
  assert in_place() == 51
}

test "a record holding a sender moved into a callee that matches it in a tuple" {
  assert in_callee() == 51
}
"#;

#[test]
fn a_record_holding_a_sender_moved_into_a_tuple_match_is_dropped_with_it() {
    use crate::borrowed_reads_adversary::{evaluator_failures, source, target_failure};
    let file = source("handle_tuple", HANDLE_TUPLE);
    let evaluator = evaluator_failures(&file);
    assert!(evaluator.is_empty(), "evaluator:\n{}", evaluator.join("\n"));
    if let Some(failure) = target_failure(&file, "rust", 2) {
        panic!("{failure}");
    }
    let llvm = target_failure(&file, "llvm", 2).expect("the LLVM gap is closed: update this test");
    assert_eq!(llvm.matches("left: 50").count(), 2, "{llvm}");
}
