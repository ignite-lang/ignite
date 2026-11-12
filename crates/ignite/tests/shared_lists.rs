//! Unit `rust-shared-payload-lists` (`docs/value-copies.md` 3.4, design
//! D; `docs/slice.md` section 6): a list that is the payload of an enum
//! variant or a field of a record shares its elements with the copies of
//! the value that holds it on the Rust target.
//!
//! `fixtures/shared_lists.ignite` changes such a list through one holder
//! in every way the language has and checks the other holder; its inline
//! tests run on the evaluator and both targets
//! ([`each_change_through_one_holder_leaves_the_other_on_every_consumer`])
//! and are units of the differential corpus.
//!
//! [`COUNTS`] appended to it is one executable, `shared-counts FORM M N`,
//! which builds an enum value `Words` of `M` texts, an enum value `Seq` of
//! `M` rows that each hold a list, and a record `Shelf` with both lists as
//! fields, then does `N` times the operation `FORM`. Its allocations less
//! those of the form `none` (the same values, no operation) over `N` are
//! the allocations of one operation, counted with valgrind memcheck's
//! `total heap usage` line as `value_copies::` counts them. An operation
//! that copies the texts of a list once makes `M` allocations more at
//! `M = 200` than at `M = 10` for the 190 texts; one that copies nothing
//! makes as many at both sizes.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::common::Target;
use crate::todo;

/// Operations each run makes.
const N: u64 = 1000;

/// The counting executable appended to the fixture.
const COUNTS: &str = r#"
type Counts {
  command_line: CommandLine
  executable: Text
}

fn number(t: Text) -> Int = match Int.parse(t) {
  Some(n) => n
  None => 0
}

fn measure(form: Text, m: Int, n: Int) -> Int {
  let v = Words(words(m, "w"))
  let r = Seq(rows(m))
  let s = shelf(m)
  var u = grown(same_val(v), "y")
  var c = same_shelf(s)
  c.words.push("y")
  var total = 0
  var i = 0
  if form == "copy-payload" {
    while i < n {
      let c = same_val(v)
      total = total + size(c)
      i = i + 1
    }
  } else if form == "copy-field" {
    while i < n {
      let c = same_shelf(s)
      total = total + c.words.len() + c.rows.len()
      i = i + 1
    }
  } else if form == "change-payload" {
    while i < n {
      let c = pushed(same_val(v), "x")
      total = total + size(c)
      i = i + 1
    }
  } else if form == "change-field" {
    while i < n {
      var c = same_shelf(s)
      c.words.push("x")
      total = total + c.words.len()
      i = i + 1
    }
  } else if form == "clear-then-push" {
    while i < n {
      var c = same_shelf(s)
      c.words.clear()
      c.words.push("x")
      total = total + c.words.len()
      i = i + 1
    }
  } else if form == "unshared-payload" {
    while i < n {
      u = grown(u, "x")
      i = i + 1
    }
  } else if form == "unshared-field" {
    while i < n {
      c.words.push("x")
      i = i + 1
    }
  } else if form == "take-payload-rows" {
    while i < n {
      let l = rows_of(r)
      total = total + l.len()
      i = i + 1
    }
  } else if form == "take-payload-words" {
    while i < n {
      let l = words_of(v)
      total = total + l.len()
      i = i + 1
    }
  } else if form == "take-field-rows" {
    while i < n {
      let l: List<Val> = s.rows
      total = total + l.len()
      i = i + 1
    }
  } else if form == "take-field-words" {
    while i < n {
      let l: List<Text> = s.words
      total = total + l.len()
      i = i + 1
    }
  }
  total + size(v) + size(r) + s.words.len() + size(u) + c.words.len()
}

fn Counts.main(self: Counts) -> Result<Unit, IoError> {
  let arguments = self.command_line.arguments()
  let m = number(arguments[1])
  let n = number(arguments[2])
  log("{arguments[0]} {m} {n}: {measure(arguments[0], m, n)}")
  return Ok(Unit)
}

system SharedCounts {
  resource command_line: CommandLine
  component counts: Counts { command_line = command_line, executable = "shared-counts" }
}
"#;

/// The counting executable, built for the Rust target.
fn counting() -> PathBuf {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/shared_lists.ignite");
    let text = std::fs::read_to_string(&fixture).expect("the fixture");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("shared-lists-source");
    std::fs::create_dir_all(&dir).expect("the source directory");
    let source = dir.join("shared_lists.ignite");
    std::fs::write(&source, text + COUNTS).expect("write the source");
    todo::build(&source, "shared-lists", Target::Rust, "shared-counts")
}

/// The allocations of `binary form m N` run to its end under memcheck.
fn allocations(binary: &Path, form: &str, m: u64) -> u64 {
    let log = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "shared-lists-{form}-{m}-{}.log",
        std::process::id()
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
    assert!(
        output.status.success(),
        "{form} {m}: {:?}: {}",
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

/// Per kind (an enum payload, a record field): copying a value holding a
/// list of M elements allocates nothing; the first change after a share
/// copies the elements once; a change of an unshared list copies nothing;
/// a payload or field taken out into a list of its own copies its
/// elements once, each element that holds a list by an increment;
/// clearing a shared list copies nothing (it takes the empty storage).
#[test]
fn a_shared_list_is_copied_once_at_its_first_change_and_never_at_a_copy() {
    let status = Command::new("valgrind")
        .args(["--tool=memcheck", "--log-file=/dev/null", "true"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap_or_else(|e| panic!("valgrind is needed to count allocations and did not run: {e}"));
    assert!(status.success(), "valgrind did not run `true`: {status:?}");
    let binary = counting();
    let none = [
        allocations(&binary, "none", 10),
        allocations(&binary, "none", 200),
    ];
    let per = |form: &str| -> (f64, f64) {
        let small = allocations(&binary, form, 10).saturating_sub(none[0]) as f64 / N as f64;
        let large = allocations(&binary, form, 200).saturating_sub(none[1]) as f64 / N as f64;
        println!("{form}: {small:.3} allocations per operation at M = 10, {large:.3} at M = 200");
        (small, large)
    };
    // What one operation may allocate more at M = 200 than at M = 10: 0
    // (nothing copied, or only increments) or the 190 texts copied once.
    let forms: [(&str, f64); 11] = [
        ("copy-payload", 0.0),
        ("copy-field", 0.0),
        ("change-payload", 190.0),
        ("change-field", 190.0),
        ("clear-then-push", 0.0),
        ("unshared-payload", 0.0),
        ("unshared-field", 0.0),
        ("take-payload-rows", 0.0),
        ("take-payload-words", 190.0),
        ("take-field-rows", 0.0),
        ("take-field-words", 190.0),
    ];
    let mut failures = Vec::new();
    for (form, more) in forms {
        let (small, large) = per(form);
        if ((large - small) - more).abs() >= 0.5 {
            failures.push(format!(
                "{form}: {small:.3} allocations per operation at M = 10, {large:.3} at M = 200, \
                 not {more} more"
            ));
        }
        if form.starts_with("copy") && large >= 0.5 {
            failures.push(format!("{form}: a copy allocates {large:.3} times"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Every inline test of `fixtures/shared_lists.ignite` passes on the
/// evaluator and both targets.
#[test]
fn each_change_through_one_holder_leaves_the_other_on_every_consumer() {
    crate::borrowed_reads_adversary::agree(
        "shared_lists",
        include_str!("fixtures/shared_lists.ignite"),
        9,
    );
}
