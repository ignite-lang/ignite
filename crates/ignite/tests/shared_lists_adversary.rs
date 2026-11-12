//! Adversary (unit `rust-shared-payload-lists`, `docs/slice.md` section 6,
//! design D of `docs/value-copies.md`): programs that hold the unit to its
//! own claims where its tests do not.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::borrowed_reads_adversary::{source, target_failure};
use crate::common::Target;
use crate::todo;

/// A record that holds a list of itself, 50,000 levels deep, built and
/// dropped. At the unit's base (`ea284a87`) the Rust target ran it (the
/// record chain overflowed the 8 MiB main stack between 70,000 and 80,000
/// levels); with the list behind `ignite_io::List` each level costs the
/// drop of the `Option<Arc<Vec<T>>>` on top of the `Vec`'s, and the same
/// program aborts with "has overflowed its stack" between 30,000 and
/// 40,000 levels. The LLVM target runs it. The evaluator is not asked: its
/// own drop of the value runs on the test thread.
/// Depth 25,000: the Rust target now overflows between 35,000 and 40,000 (`docs/slice.md` 6).
const DEEP_RECORDS: &str = r#"package deep_records

type Node {
  tag: Int
  kids: List<Node>
}

fn chain(n: Int) -> Node {
  var cur = Node { tag = 0, kids = [] }
  var i = 1
  while i < n {
    cur = Node { tag = i, kids = [cur] }
    i = i + 1
  }
  cur
}

test "a record holding a list of itself 25000 levels deep is built and dropped" {
  let c = chain(25000)
  assert c.tag == 24999 && c.kids.len() == 1
}
"#;

#[test]
fn adversary_a_deep_chain_of_records_holding_lists_drops_where_it_did_before() {
    let file = source("deep_records", DEEP_RECORDS);
    let failures: Vec<String> = ["rust", "llvm"]
        .into_iter()
        .filter_map(|target| target_failure(&file, target, 1))
        .collect();
    assert!(
        failures.is_empty(),
        "deep_records:\n{}",
        failures.join("\n")
    );
}

/// Operations each counting run makes.
const N: u64 = 1000;

/// A read of a list payload of a value no local holds, in the shape of the
/// first user's lookups (`docs/value-copies.md` form 2: a lookup returns a
/// copy of the value found, the caller matches it and only measures the
/// list). `docs/slice.md` section 6 says a copy of the value is an
/// increment and a read derefs (`(*r.items)`, a payload binding
/// `let l = &**l;`); only a payload "taken out into a list of its own"
/// is still copied. `read-field` (the field of a copy, `same(s).words`) is
/// read in place; `read-payload` (the payload of a copy, matched) is not:
/// the arm binds `let l = ::ignite_io::List::into_vec(l);`, which copies
/// every element (here a text each), since the value it came from still
/// shares them.
const READS: &str = r#"package shared_reads

enum Val {
  Nil
  Txt(Text)
  Seq(List<Val>)
}

type Shelf {
  id: Int
  words: List<Val>
}

fn items(m: Int) -> List<Val> {
  var out: List<Val> = []
  var i = 0
  while i < m {
    out.push(Txt("t{i}"))
    i = i + 1
  }
  out
}

fn lookup(v: Val, k: Int) -> Option<Val> = if k >= 0 { Some(v) } else { None }

fn same(s: Shelf) -> Shelf = s

fn number(t: Text) -> Int = match Int.parse(t) {
  Some(n) => n
  None => 0
}

fn measure(form: Text, m: Int, n: Int) -> Int {
  let v = Seq(items(m))
  let s = Shelf { id = 1, words = items(m) }
  var total = 0
  var i = 0
  if form == "read-field" {
    while i < n {
      total = total + same(s).words.len()
      i = i + 1
    }
  } else if form == "read-payload" {
    while i < n {
      total = total + match lookup(v, i) {
        Some(Seq(l)) => l.len()
        _ => 0
      }
      i = i + 1
    }
  } else if form == "moved-field-change" {
    var cur = Shelf { id = 0, words = items(m) }
    while i < n {
      var next = Shelf { id = i, words = cur.words }
      next.words.push(Nil)
      cur = next
      i = i + 1
    }
    total = cur.words.len()
  }
  total
}

type Counts {
  command_line: CommandLine
  executable: Text
}

fn Counts.main(self: Counts) -> Result<Unit, IoError> {
  let arguments = self.command_line.arguments()
  let m = number(arguments[1])
  let n = number(arguments[2])
  log("{arguments[0]} {m} {n}: {measure(arguments[0], m, n)}")
  return Ok(Unit)
}

system SharedReads {
  resource command_line: CommandLine
  component counts: Counts { command_line = command_line, executable = "shared-reads" }
}
"#;

/// The allocations of `binary form m N` run to its end under memcheck.
fn allocations(binary: &Path, form: &str, m: u64) -> u64 {
    static RUN: AtomicUsize = AtomicUsize::new(0);
    let log = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "shared-reads-{form}-{m}-{}-{}.log",
        std::process::id(),
        RUN.fetch_add(1, Ordering::Relaxed)
    ));
    let output = Command::new("valgrind")
        .arg("--tool=memcheck")
        .arg(format!("--log-file={}", log.display()))
        .arg(binary)
        .args([form, &m.to_string(), &N.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .expect("run valgrind");
    assert!(output.status.success(), "{form} {m}: {:?}", output.status);
    let text = std::fs::read_to_string(&log).expect("the valgrind log");
    let _ = std::fs::remove_file(&log);
    text.lines()
        .find_map(|line| line.split_once("total heap usage: ").map(|(_, rest)| rest))
        .and_then(|usage| usage.split_whitespace().next())
        .and_then(|count| count.replace(',', "").parse().ok())
        .unwrap_or_else(|| panic!("no heap usage in the valgrind log:\n{text}"))
}

/// The counting executable, built once for the tests that run it.
fn counting() -> &'static Path {
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    BUILT.get_or_init(|| {
        let file = source("shared_reads", READS);
        todo::build(&file, "shared-reads", Target::Rust, "shared-reads")
    })
}

/// The allocations of one operation `form` at M = 10 and at M = 200.
fn per_operation(form: &str) -> (f64, f64) {
    let binary = counting();
    let at = |m: u64| {
        allocations(binary, form, m).saturating_sub(allocations(binary, "none", m)) as f64
            / N as f64
    };
    (at(10), at(200))
}

#[test]
fn adversary_a_payload_of_a_copy_matched_and_only_read_copies_no_element() {
    let mut failures = Vec::new();
    for form in ["read-field", "read-payload"] {
        let (small, large) = per_operation(form);
        if (large - small).abs() >= 0.5 {
            failures.push(format!(
                "{form}: {small:.3} allocations per read at M = 10, {large:.3} at M = 200: \
                 the read copies the elements"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A list field moved into a new record (`Shelf { words = cur.words, .. }`
/// where `cur` is assigned before it is read again) and changed there: the
/// moves analysis moves the field, so the new record holds the only copy
/// and its change copies nothing. Green now; with `shared_value` copying
/// (`.clone()`, an increment) where the moves analysis says the field
/// moves, the old record keeps a share until `cur = next` drops it and
/// every push copies the list, and no other test of `ignite-rustgen` or of
/// `shared_lists::`, `value_copies::`, `slice16::`, `borrowed_reads::`,
/// `copies::`, `terminal::`, `todo::` or `mysql::` fails (adversary
/// pass on 3de42165).
#[test]
fn adversary_a_field_moved_into_a_new_record_and_changed_copies_nothing() {
    let (small, large) = per_operation("moved-field-change");
    assert!(
        (large - small).abs() < 0.5,
        "moved-field-change: {small:.3} allocations per round at M = 10, {large:.3} at M = 200"
    );
}

/// A shared list field of a value no local holds (`row(id).cells`) as the
/// tail of a fn, a block, a match arm and a branch: `Generator::result`
/// writes it with `List::into_vec`. Green now; with that branch of
/// `result` switched off the Rust target refuses the program (E0507, a
/// move out of the `List`'s deref), and no other test of `ignite-rustgen`
/// or of `shared_lists::`, `value_copies::`, `slice16::`,
/// `borrowed_reads::`, `copies::`, `terminal::`, `todo::` or `mysql::`
/// fails (adversary pass on 3de42165).
const RESULT_FIELDS: &str = r#"package result_fields

enum Cell {
  Nil
  Txt(Text)
}

type Row {
  id: Int
  cells: List<Cell>
}

fn row(id: Int) -> Row = Row { id = id, cells = [Txt("r{id}"), Nil] }

fn cells_of(id: Int) -> List<Cell> = row(id).cells

fn block_tail(id: Int) -> List<Cell> {
  let n = id + 1
  row(n).cells
}

fn arm_tail(id: Int) -> List<Cell> = match id {
  0 => []
  _ => row(id).cells
}

fn branch_tail(id: Int) -> List<Cell> = if id > 0 { row(id).cells } else { [] }

test "a field of a value no local holds is the tail of a fn, a block, an arm and a branch" {
  assert cells_of(1) == [Txt("r1"), Nil]
  assert block_tail(1) == [Txt("r2"), Nil]
  assert arm_tail(3) == [Txt("r3"), Nil] && arm_tail(0).is_empty()
  var b = branch_tail(4)
  b.push(Txt("x"))
  assert b.len() == 3 && branch_tail(0).is_empty()
}
"#;

#[test]
fn adversary_a_field_of_a_temporary_in_a_result_position_compiles() {
    crate::borrowed_reads_adversary::agree("result_fields", RESULT_FIELDS, 1);
}
