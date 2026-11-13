//! Story `shared-list-costs-back` (`docs/slice.md` section 6): a program
//! that never copies a list held by a record field or an enum payload pays
//! for it what it paid before the shared lists, when such a list was a
//! plain `Vec`: no block for an empty list, one block (the buffer) for a
//! non-empty one.
//!
//! [`COSTS`] is one executable, `list-costs FORM N`, which builds `N`
//! records or enum values of the form `FORM`, reads each list's length and
//! drops the value; none of them is ever copied. Its allocations less those
//! of the form `none` (the same loop, no value built) over `N` are the
//! allocations of one value, counted with valgrind memcheck's `total heap
//! usage` line as `shared_lists::` counts them. The elements are `Int`s,
//! which allocate nothing of their own, so every allocation counted is a
//! block of the list.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::common::Target;
use crate::todo;

/// Values each run builds.
const N: u64 = 1000;

/// The synthetic program: records with list fields, one with an empty
/// default, and an enum with a list payload; each value is built, read and
/// dropped, never copied. Its inline test gives the values the counting
/// executable relies on, on the evaluator and both targets.
const COSTS: &str = r#"package list_costs

type Row {
  id: Int
  cells: List<Int>
}

type Tagged {
  id: Int
  cells: List<Int> = []
  marks: List<Int> = [7, 8]
}

enum Bag {
  Nothing
  Items(List<Int>)
}

fn size(b: Bag) -> Int = match b {
  Items(l) => l.len()
  Nothing => 0
}

fn built(i: Int) -> List<Int> {
  var l: List<Int> = List.with_capacity(2)
  l.push(i)
  l.push(i + 1)
  l
}

fn measure(form: Text, n: Int) -> Int {
  var total = 0
  var i = 0
  if form == "empty-literal" {
    while i < n {
      let r = Row { id = i, cells = [] }
      total = total + r.id + r.cells.len()
      i = i + 1
    }
  } else if form == "empty-default" {
    while i < n {
      let r = Tagged { id = i, marks = [] }
      total = total + r.id + r.cells.len() + r.marks.len()
      i = i + 1
    }
  } else if form == "empty-payload" {
    while i < n {
      total = total + i + size(Items([]))
      i = i + 1
    }
  } else if form == "literal" {
    while i < n {
      let r = Row { id = i, cells = [i, i + 1, i + 2] }
      total = total + r.id + r.cells.len()
      i = i + 1
    }
  } else if form == "literal-default" {
    while i < n {
      let r = Tagged { id = i }
      total = total + r.id + r.cells.len() + r.marks.len()
      i = i + 1
    }
  } else if form == "literal-payload" {
    while i < n {
      total = total + i + size(Items([i, i + 1]))
      i = i + 1
    }
  } else if form == "built" {
    while i < n {
      let r = Row { id = i, cells = built(i) }
      total = total + r.id + r.cells.len()
      i = i + 1
    }
  } else {
    while i < n {
      total = total + i
      i = i + 1
    }
  }
  total
}

test "the forms build the lists they name" {
  assert measure("empty-literal", 3) == 3
  assert measure("empty-default", 3) == 3
  assert measure("empty-payload", 3) == 3
  assert measure("literal", 3) == 12
  assert measure("literal-default", 3) == 9
  assert measure("literal-payload", 3) == 9
  assert measure("built", 3) == 9
  assert measure("none", 3) == 3
  let t = Tagged { id = 1 }
  assert t.cells == [] && t.marks == [7, 8]
  var u = Tagged { id = 2, cells = [] }
  u.cells.push(4)
  u.marks.push(9)
  assert u.cells == [4] && u.marks == [7, 8, 9] && t.marks == [7, 8]
  assert size(Items([])) == 0 && size(Items([1, 2, 3])) == 3 && size(Nothing) == 0
}

fn number(t: Text) -> Int = match Int.parse(t) {
  Some(n) => n
  None => 0
}

type Costs {
  command_line: CommandLine
  executable: Text
}

fn Costs.main(self: Costs) -> Result<Unit, IoError> {
  let arguments = self.command_line.arguments()
  let n = number(arguments[1])
  log("{arguments[0]} {n}: {measure(arguments[0], n)}")
  return Ok(Unit)
}

system ListCosts {
  resource command_line: CommandLine
  component costs: Costs { command_line = command_line, executable = "list-costs" }
}
"#;

/// The counting executable, built once for the Rust target.
fn counting() -> &'static Path {
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    BUILT.get_or_init(|| {
        let file = crate::borrowed_reads_adversary::source("list_costs_counting", COSTS);
        todo::build(&file, "list-costs", Target::Rust, "list-costs")
    })
}

/// The allocations of `binary form N` run to its end under memcheck.
fn allocations(binary: &Path, form: &str) -> u64 {
    static RUN: AtomicUsize = AtomicUsize::new(0);
    let log = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "list-costs-{form}-{}-{}.log",
        std::process::id(),
        RUN.fetch_add(1, Ordering::Relaxed)
    ));
    let output = Command::new("valgrind")
        .arg("--tool=memcheck")
        .arg(format!("--log-file={}", log.display()))
        .arg(binary)
        .args([form, &N.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .unwrap_or_else(|e| panic!("valgrind is needed to count allocations and did not run: {e}"));
    assert!(
        output.status.success(),
        "{form}: {:?}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let text = std::fs::read_to_string(&log).expect("the valgrind log");
    let _ = std::fs::remove_file(&log);
    text.lines()
        .find_map(|line| line.split_once("total heap usage: ").map(|(_, rest)| rest))
        .and_then(|usage| usage.split_whitespace().next())
        .and_then(|count| count.replace(',', "").parse().ok())
        .unwrap_or_else(|| panic!("no heap usage in the valgrind log:\n{text}"))
}

/// Each form against its bar, allocations per value built: the failures.
fn over(forms: &[(&str, f64)]) -> Vec<String> {
    let binary = counting();
    let none = allocations(binary, "none");
    let mut failures = Vec::new();
    for (form, bar) in forms {
        let per = allocations(binary, form).saturating_sub(none) as f64 / N as f64;
        println!("{form}: {per:.3} allocations per value, a plain Vec {bar}");
        if (per - bar).abs() >= 0.5 {
            failures.push(format!(
                "{form}: {per:.3} allocations per value, a plain Vec made {bar}"
            ));
        }
    }
    failures
}

/// An empty list field, an empty default and an empty payload hold no
/// block: building the value allocates nothing for the list.
#[test]
fn an_empty_list_field_default_or_payload_allocates_nothing() {
    let failures = over(&[
        ("empty-literal", 0.0),
        ("empty-default", 0.0),
        ("empty-payload", 0.0),
    ]);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A list literal in a field, a default or a payload is one block, as a
/// `Vec` built from the literal is.
#[test]
fn a_list_literal_in_a_field_default_or_payload_is_one_block() {
    let failures = over(&[
        ("literal", 1.0),
        ("literal-default", 1.0),
        ("literal-payload", 1.0),
    ]);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A non-empty list built in a local (one buffer, its capacity reserved)
/// and moved into a field that no other value shares holds one block.
#[test]
fn a_non_empty_unshared_list_holds_one_block() {
    let failures = over(&[("built", 1.0)]);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The inline test of [`COSTS`] passes on the evaluator and both targets.
#[test]
fn the_forms_agree_on_every_consumer() {
    crate::borrowed_reads_adversary::agree("list_costs", COSTS, 1);
}
