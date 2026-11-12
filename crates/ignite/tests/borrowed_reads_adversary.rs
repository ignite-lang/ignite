//! Adversary (story `std-mysql-client`, unit `mysql-cost`): programs whose
//! element reads the Rust target now lends (`docs/slice.md` 20.3.7,
//! "Element reads are lent") or that the LLVM target reads in place, run
//! by the three consumers of a typed program. The evaluator is the
//! reference for values (`differential::`); each target runs the
//! program's inline tests with `ignite test` and must pass them all, which
//! means it compiles and gives the evaluator's values.

use std::path::{Path, PathBuf};
use std::process::Command;

use ignite_eval::{Evaluator, Fuel};
use ignite_syntax::ast::Decl;

use crate::common;

/// `program` written as `<name>.ignite` in a directory of its own.
pub(crate) fn source(name: &str, program: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("borrowed-reads-adversary")
        .join(name);
    std::fs::create_dir_all(&dir).expect("the source directory");
    let file = dir.join(format!("{name}.ignite"));
    std::fs::write(&file, program).expect("write the program");
    file
}

/// The failures of the evaluator on the inline tests of `file`: one line
/// per test that does not pass.
fn evaluator_failures(file: &Path) -> Vec<String> {
    let loaded = ignite::load(file, None).unwrap_or_else(|e| panic!("{e}"));
    let modules = loaded.modules();
    let module = loaded.types().module.clone();
    let names: Vec<String> = loaded
        .program()
        .decls
        .iter()
        .filter_map(|decl| match decl {
            Decl::Test(t) => Some(t.name.name.clone()),
            _ => None,
        })
        .collect();
    assert!(!names.is_empty(), "no inline test in {}", file.display());
    let mut evaluator = Evaluator::new(&modules, Fuel::with_units(10 * Fuel::DEFAULT_LIMIT));
    names
        .iter()
        .filter_map(|name| match evaluator.run_test(&module, name) {
            Ok(()) => None,
            Err(error) => Some(format!("{name}: {error:?}")),
        })
        .collect()
}

/// `ignite test <file> --target <target>`: `None` when every one of its
/// `count` tests passes, else its exit status and output.
pub(crate) fn target_failure(file: &Path, target: &str, count: usize) -> Option<String> {
    let out = file
        .parent()
        .expect("the source directory")
        .join(format!("out-{target}"));
    let mut command = Command::new(env!("CARGO_BIN_EXE_ignite"));
    command
        .arg("test")
        .arg(file)
        .args(["--target", target, "--out"])
        .arg(&out)
        .arg("--runtime")
        .arg(common::runner_archive());
    if target == "rust" {
        command.arg("--target-dir").arg(out.join("cargo"));
    }
    let output = command.output().expect("run ignite test");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let passed = format!("test result: ok. {count} passed; 0 failed");
    if output.status.code() == Some(0) && stdout.contains(&passed) {
        return None;
    }
    let errors: Vec<&str> = stderr
        .lines()
        .chain(stdout.lines())
        .filter(|line| {
            line.starts_with("error")
                || line.contains("FAILED")
                || line.contains("left:")
                || line.contains("right:")
        })
        .collect();
    Some(format!(
        "{target}: exit {:?}\n{}",
        output.status.code(),
        errors.join("\n")
    ))
}

/// Every inline test of `program` passes on the evaluator, the Rust target
/// and the LLVM target.
fn agree(name: &str, program: &str, count: usize) {
    let file = source(name, program);
    let mut failures = evaluator_failures(&file);
    for target in ["rust", "llvm"] {
        failures.extend(target_failure(&file, target, count));
    }
    assert!(failures.is_empty(), "{name}:\n{}", failures.join("\n"));
}

/// A builtin whose receiver is an element `l[i]` and whose argument
/// replaces the list `l`: the receiver is read before the argument
/// (`differential::the_evaluator_reads_a_receiver_before_its_arguments`),
/// so the call sees the element as it was. At 4bcda2ab `ref_arg` lent
/// the element (`checked::at(&l, 0, ..)`) to `checked::slice`,
/// `text::pieces`, `text::index_of` and the outer `checked::at` of
/// `l[0].at(j)`, whose argument assigns `l` while the loan is held, and
/// rustc refused it with E0506. Now only callers that check what is
/// evaluated during the loan lend an element (`lent_arg`, `lent_across`);
/// these copy it (`&checked::at(&l, 0, ..).clone()`), as before the unit.
const RECEIVER_THEN_ARGUMENT: &str = r#"package receiver_then_argument

fn slice_of_element() -> Int {
  var l = [[1, 2, 3], [4]]
  let s = l[0].slice(0, if true {
    l = [[7]]
    2
  } else {
    2
  })
  s[0] * 10 + s[1] + l.len() * 100
}

fn pieces_of_element() -> Int {
  var l = ["a,b,c", "d"]
  let p = l[0].pieces(if true {
    l = ["x"]
    ","
  } else {
    ","
  })
  p.len() + l.len() * 100
}

fn index_of_in_element() -> Int {
  var l = [b"hello", b"x"]
  match l[0].index_of(if true {
    l = [b"zz"]
    b"l"
  } else {
    b"l"
  }, 0) {
    Some(i) => i + l.len() * 100
    None => -1
  }
}

fn at_of_element() -> Int {
  var l = [[1, 2, 3], [4]]
  let k = l[0].at(if true {
    l = [[7, 8, 9]]
    1
  } else {
    1
  })
  k + l.len() * 100
}

fn index_of_in_slice_of_element() -> Int {
  var l = [b"hello", b"x"]
  match l[0].slice(0, 4).index_of(if true {
    l = [b"zz"]
    b"l"
  } else {
    b"l"
  }, 0) {
    Some(i) => i + l.len() * 100
    None => -1
  }
}

test "a slice of an element is taken before its bound replaces the list" {
  assert slice_of_element() == 112
}

test "an element is split before its separator replaces the list" {
  assert pieces_of_element() == 103
}

test "an element is searched before its needle replaces the list" {
  assert index_of_in_element() == 102
}

test "an element is indexed before its index replaces the list" {
  assert at_of_element() == 102
}

test "a slice of an element is searched before its needle replaces the list" {
  assert index_of_in_slice_of_element() == 102
}
"#;

#[test]
fn adversary_an_element_receiver_of_a_builtin_whose_argument_changes_the_list() {
    agree("receiver_then_argument", RECEIVER_THEN_ARGUMENT, 5);
}

/// One program per guard of the change, each where the guard decides:
/// the body, arm, argument or operand changes the list the element or the
/// `for` would borrow from. Green now; with one guard switched off it goes
/// red, which no other test of `ignite-rustgen`, `ignite-llvm`,
/// `borrowed_reads::` or the differential corpus did for four of them
/// (adversary pass on 4bcda2ab): `reads_element_in_place`'s argument
/// check (`receiver_changed`), `ref_args`' check of a lent element
/// against later arguments (`argument_changed`), `compared`'s check of the
/// later operand (`operand_changed`), and on LLVM `unchanged_in` for a
/// `match` on `l[i]` (`scrutinee_changed`). The other three (`for` over a
/// list, over integers, over an element) were covered already.
const GUARDS: &str = r#"package guards

enum V {
  N
  S(Text)
}

fn weigh(a: Text, b: Text) -> Int = a.len() * 10 + b.len()

fn pairs_changed() -> Int {
  var r = [("a", 1), ("bb", 2)]
  var n = 0
  for (t, k) in r {
    r[1] = ("zzzz", 10)
    n = n + t.len() + k
  }
  n
}

fn ints_changed() -> Int {
  var l = [1, 2, 3]
  var n = 0
  for x in l {
    l.push(x * 10)
    n = n + x
  }
  n + l.len() * 100
}

fn element_changed() -> Int {
  var l = [["a", "bb"], ["c"]]
  var n = 0
  for t in l[0] {
    l[0] = ["zzzzz"]
    n = n + t.len()
  }
  n
}

fn scrutinee_changed() -> Int {
  var l = [S("abc"), N]
  match l[0] {
    S(t) => {
      l[0] = S("zzzzz")
      t.len()
    }
    N => 0
  }
}

fn receiver_changed() -> Int {
  var l = ["ab", "c"]
  if l[0].starts_with(if true {
    l = ["x"]
    "a"
  } else {
    ""
  }) {
    1
  } else {
    2
  }
}

fn argument_changed() -> Int {
  var l = ["ab", "c"]
  weigh(l[0], if true {
    l[0] = "xyz"
    "q"
  } else {
    ""
  })
}

fn operand_changed() -> Bool {
  var l = ["a", "b"]
  l[0] == if true {
    l[0] = "b"
    l[0]
  } else {
    ""
  }
}

test "a tuple-pattern for over a list its body changes iterates the list as it was" {
  assert pairs_changed() == 6
}

test "a for over integers whose body pushes onto the list iterates the list as it was" {
  assert ints_changed() == 606
}

test "a for over an element its body replaces iterates the element as it was" {
  assert element_changed() == 3
}

test "a match on an element its arm replaces binds the element as it was" {
  assert scrutinee_changed() == 3
}

test "a read-only method on an element reads it before its argument replaces the list" {
  assert receiver_changed() == 1
}

test "an element argument is read before a later argument changes the list" {
  assert argument_changed() == 21
}

test "an element operand is read before the other operand changes the list" {
  assert !operand_changed()
}
"#;

#[test]
fn adversary_each_guard_of_a_lent_element_decides_one_program() {
    agree("guards", GUARDS, 7);
}
