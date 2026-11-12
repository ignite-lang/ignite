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
fn source(name: &str, program: &str) -> PathBuf {
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
fn target_failure(file: &Path, target: &str, count: usize) -> Option<String> {
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
/// so the call sees the element as it was. Before the change the Rust
/// target copied the element (`&checked::at(&l, 0, ..).clone()`); now
/// `ref_arg` lends it (`checked::at(&l, 0, ..)`) to `checked::slice`,
/// `text::pieces`, `text::index_of` and the outer `checked::at` of
/// `l[0].at(j)`, whose argument assigns `l` while the loan is held:
/// rustc refuses it with E0506. `ref_args`, `compared` and
/// `reads_element_in_place` check the later arguments for a change of the
/// list; these builtin receivers go through `ref_arg` without that check.
/// All three consumers passed it before the change.
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

/// The read-only methods of `reads_element_in_place` on an element whose
/// argument replaces the list: the Rust target copies the element (the
/// guard that no argument changes the list) and gives the evaluator's
/// value. The LLVM target reads the receiver in place and the argument's
/// assignment is seen by the call: `starts_with` answers for `"x"`, not
/// `"ab"`, and `strip_prefix` for `"zzzzzz"`, not `"abc"`. Present at the
/// base commit too (the LLVM lowering of these receivers is not part of
/// the change).
const LLVM_RECEIVER_THEN_ARGUMENT: &str = r#"package llvm_receiver_then_argument

fn starts_with_of_element() -> Int {
  var l = ["ab", "c"]
  if l[0].starts_with(if true {
    l[0] = "x"
    "a"
  } else {
    ""
  }) {
    1
  } else {
    2
  }
}

fn strip_prefix_of_element() -> Int {
  var l = ["abc", "c"]
  match l[0].strip_prefix(if true {
    l[0] = "zzzzzz"
    "a"
  } else {
    ""
  }) {
    Some(t) => t.len()
    None => 100
  }
}

test "starts_with reads the element before its argument changes it" {
  assert starts_with_of_element() == 1
}

test "strip_prefix reads the element before its argument changes it" {
  assert strip_prefix_of_element() == 2
}
"#;

#[test]
fn adversary_a_read_only_method_on_an_element_whose_argument_changes_it() {
    agree(
        "llvm_receiver_then_argument",
        LLVM_RECEIVER_THEN_ARGUMENT,
        2,
    );
}

/// `l[0][j]` whose index `j` replaces `l`, passed to a parameter by
/// reference. The evaluator reads the index first and the element of the
/// new list (`"zz"`, 2); both targets read `l[0]` first (3) at the base
/// commit, and the LLVM target still does. The Rust target compiled it at
/// the base commit (the inner element copied); now the inner `l[0]` is
/// lent across the index that assigns `l`, and rustc refuses it with
/// E0506.
const INDEX_REPLACES_THE_OUTER_LIST: &str = r#"package index_replaces_the_outer_list

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

test "an element of an element is read after its index replaces the list" {
  assert element_of_element() == 2
}
"#;

#[test]
fn adversary_an_index_of_an_element_that_replaces_the_list() {
    agree(
        "index_replaces_the_outer_list",
        INDEX_REPLACES_THE_OUTER_LIST,
        1,
    );
}
