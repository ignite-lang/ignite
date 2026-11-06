//! Slice 16, language gaps wave A (`docs/slice.md` section 20): the
//! fixtures under `tests/fixtures/slice16/` run through `ignite test` on
//! both targets.

use crate::common;

use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/slice16")
        .join(name)
}

fn out_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("slice16")
        .join(name)
}

/// Row 8 (20.4.1): the closure shapes of probes p02, p08 and p04b and the
/// actors §5.1 handle loop pass on both targets with no change to either
/// lowering.
#[test]
fn closure_shapes_pass_on_both_targets() {
    let dir = out_dir("closures");
    let output = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("test")
        .arg(fixture("closures.ignite"))
        .args(["--target", "all", "--out"])
        .arg(&dir)
        .arg("--target-dir")
        .arg(dir.join("cargo"))
        .arg("--runtime")
        .arg(common::runner_archive())
        .output()
        .expect("run ignite test");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(
        stdout
            .matches("test result: ok. 13 passed; 0 failed;")
            .count(),
        2,
        "{stdout}"
    );
    assert!(!stdout.contains("FAILED"), "{stdout}");
}

/// 20.4.2 on both targets for the callee forms `closures.ignite` and
/// `calls.ignite` leave out: the evaluation order (callee first, then the
/// arguments left to right) and an indexed callee whose list's element type
/// a later `push` decides, which `ignite check` accepts; also a fn value
/// bound from such a list, and a `Map.empty()` and a `None` that a later
/// `put` or assignment decides, whose generated `let`s carry the type.
#[test]
fn callee_forms_pass_on_both_targets() {
    let dir = out_dir("callee_forms");
    let output = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("test")
        .arg(fixture("callee_forms.ignite"))
        .args(["--target", "all", "--out"])
        .arg(&dir)
        .arg("--target-dir")
        .arg(dir.join("cargo"))
        .arg("--runtime")
        .arg(common::runner_archive())
        .output()
        .expect("run ignite test");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(
        stdout
            .matches("test result: ok. 5 passed; 0 failed;")
            .count(),
        2,
        "{stdout}"
    );
}

/// `ignite test` of `files` with `args` on the LLVM target against the
/// debug runtime, whose releases overwrite freed memory with 0xDB
/// (`docs/llvm.md` section 4): exit status and stdout.
fn llvm_test(dir: &Path, files: &[&str], args: &[&str]) -> (Option<i32>, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("test")
        .args(files.iter().map(|name| fixture(name)))
        .args(args)
        .args(["--target", "llvm", "--out"])
        .arg(dir)
        .arg("--runtime")
        .arg(common::runner_archive())
        .output()
        .expect("run ignite test");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("test result: "),
        "stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    (output.status.code(), stdout)
}

/// The seconds of the `finished in` figure of the runner's `test result:`
/// line in `stdout`.
fn finished_in(stdout: &str) -> f64 {
    let line = stdout
        .lines()
        .find(|line| line.starts_with("test result: "))
        .unwrap_or_else(|| panic!("no test result line in {stdout}"));
    let at = line.find("; finished in ").expect("a finished in figure") + "; finished in ".len();
    line[at..]
        .trim_end_matches('s')
        .parse()
        .unwrap_or_else(|e| panic!("{line}: {e}"))
}

/// Row 10 (20.3.5): R1's state threaded through a call is linear on the
/// LLVM target. Each filter runs twice and the second run's time counts;
/// 200,000 steps take under 8 times the time of 50,000 (linear is about
/// 4, a copy per step about 16 or more).
#[test]
fn llvm_state_threading_is_linear() {
    let dir = out_dir("moves_scale_llvm");
    let mut times = Vec::new();
    for filter in ["50000 steps through a call", "200000 steps through a call"] {
        let mut last = 0.0;
        for _ in 0..2 {
            let (status, stdout) = llvm_test(&dir, &["moves_scale.ignite"], &[filter]);
            assert_eq!(status, Some(0), "{filter}:\n{stdout}");
            assert!(
                stdout.contains("test result: ok. 1 passed; 0 failed; 1 filtered out;"),
                "{filter}:\n{stdout}"
            );
            last = finished_in(&stdout);
        }
        times.push(last);
    }
    let (small, large) = (times[0], times[1]);
    assert!(
        large < 8.0 * small.max(0.01),
        "200000 steps took {large:.2} s, 50000 steps {small:.2} s"
    );
}

/// Row 12 (20.3.5): moves keep value semantics on the LLVM target, built
/// against the debug runtime, so a moved state that a loop release copied
/// reads garbage if a move left it pointing at released memory. Row 18:
/// `rest.ignite` too, whose rest bindings are copies.
#[test]
fn llvm_fixtures_pass() {
    let dir = out_dir("fixtures_llvm");
    let (status, stdout) = llvm_test(&dir, &["moves.ignite", "calls.ignite", "rest.ignite"], &[]);
    assert_eq!(status, Some(0), "{stdout}");
    let results: Vec<&str> = stdout
        .lines()
        .filter(|line| line.starts_with("test result: "))
        .collect();
    assert_eq!(results.len(), 3, "one executable per file:\n{stdout}");
    assert!(
        results[0].starts_with("test result: ok. 13 passed; 0 failed;"),
        "moves.ignite:\n{stdout}"
    );
    assert!(
        results[1].starts_with("test result: ok. 6 passed; 0 failed;"),
        "calls.ignite:\n{stdout}"
    );
    assert!(
        results[2].starts_with("test result: ok. 7 passed; 0 failed;"),
        "rest.ignite:\n{stdout}"
    );
    assert!(!stdout.contains("FAILED"), "{stdout}");
}

/// `ignite test FILE --target all` of one slice-16 fixture: exit status and
/// stdout, with stderr appended for the failure message.
fn both_targets(name: &str) -> (Option<i32>, String) {
    let dir = out_dir(name.trim_end_matches(".ignite"));
    let output = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("test")
        .arg(fixture(name))
        .args(["--target", "all", "--out"])
        .arg(&dir)
        .arg("--target-dir")
        .arg(dir.join("cargo"))
        .arg("--runtime")
        .arg(common::runner_archive())
        .output()
        .expect("run ignite test");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr);
    (output.status.code(), format!("{stdout}\nstderr:\n{stderr}"))
}

/// 20.3.2, "an argument when another argument of the same call or of an
/// enclosing call ... mentions an overlapping place": an operator's left
/// operand, a field operand and an index receiver are read in place after
/// the right operand ran, so a move under the right operand that changes
/// the place shows the changed value. Every case passes with copies.
#[test]
fn held_operands_keep_their_values_on_both_targets() {
    let (status, out) = both_targets("moves_held.ignite");
    assert_eq!(status, Some(0), "{out}");
    assert_eq!(
        out.matches("test result: ok. 3 passed; 0 failed;").count(),
        2,
        "{out}"
    );
}

/// 20.3.1 and 20.3.4 (adversary, U3): a parameter its fn consumes by a
/// move on every path is declared by value on the Rust target, and is the
/// tail of an `if` branch or a `match` arm whose value is used while the
/// parameter is read afterwards. The tail is no move, so it must copy; rustc
/// must accept the program (vision X23) and both targets return the copied
/// values.
#[test]
fn consumed_parameter_tails_keep_their_values_on_both_targets() {
    let (status, out) = both_targets("moves_tails.ignite");
    assert_eq!(status, Some(0), "{out}");
    assert_eq!(
        out.matches("test result: ok. 3 passed; 0 failed;").count(),
        2,
        "{out}"
    );
}

/// 20.3.1 and 20.3.2 (adversary, U3): a local that is the tail of an `if`
/// branch or a `match` arm whose value is used, read later in the block or
/// in the next loop iteration, is no move; rustc must accept the program and
/// both targets return the copied values.
#[test]
fn local_tails_keep_their_values_on_both_targets() {
    let (status, out) = both_targets("moves_tails_local.ignite");
    assert_eq!(status, Some(0), "{out}");
    assert_eq!(
        out.matches("test result: ok. 3 passed; 0 failed;").count(),
        2,
        "{out}"
    );
}

/// 20.3.1 and 20.3.4: a pattern binding and an owned `for` element, which
/// the analysis does not track and so never moves, as the tail of an `if`
/// branch whose value is used and read afterwards, copy; rustc must accept
/// the program and both targets return the copied values.
#[test]
fn untracked_tails_keep_their_values_on_both_targets() {
    let (status, out) = both_targets("moves_tails_untracked.ignite");
    assert_eq!(status, Some(0), "{out}");
    assert_eq!(
        out.matches("test result: ok. 2 passed; 0 failed;").count(),
        2,
        "{out}"
    );
}

/// 20.3.1 (story:moves-for-pattern-bindings): owned pattern bindings, `for`
/// elements and closure parameters move at their last use; one read after
/// the tail keeps its value, a binding of a place leaves the place
/// unchanged, and a closure parameter its body moves is passed a copy.
/// rustc must accept the program and both targets agree.
#[test]
fn owned_binding_tails_move_on_both_targets() {
    let (status, out) = both_targets("moves_tails_owned.ignite");
    assert_eq!(status, Some(0), "{out}");
    assert_eq!(
        out.matches("test result: ok. 8 passed; 0 failed;").count(),
        2,
        "{out}"
    );
}

/// 20.5 (adversary, U5): rest bindings of texts, records, lists of lists,
/// variant payloads, closures, bytes and units, and a rest that drives a
/// loop reassigning its scrutinee, on both targets.
#[test]
fn rest_adversary_cases_pass_on_both_targets() {
    for (name, count) in [
        ("rest_adversary.ignite", 13),
        ("rest_adversary2.ignite", 11),
        ("rest_moves.ignite", 5),
        ("rest_eval.ignite", 1),
    ] {
        let (status, out) = both_targets(name);
        assert_eq!(status, Some(0), "{name}: {out}");
        assert_eq!(
            out.matches(&format!("test result: ok. {count} passed; 0 failed;"))
                .count(),
            2,
            "{name}: {out}"
        );
    }
}

/// 20.0 row 20 (adversary, U5): the evaluator matches the rest patterns of
/// `rest_eval.ignite` as both targets do; every `e_` fn is `true` on the
/// targets (`rest_adversary_cases_pass_on_both_targets`) and must be in the
/// evaluator.
#[test]
fn evaluator_matches_rest_patterns() {
    use ignite_eval::{Evaluator, Fuel};
    use ignite_syntax::ast::Decl;
    use ignite_types::QualName;
    let loaded = ignite::load(&fixture("rest_eval.ignite"), None).unwrap_or_else(|e| panic!("{e}"));
    let modules = loaded.modules();
    let mut evaluator = Evaluator::new(&modules, Fuel::default());
    let module = loaded.types().module.clone();
    let names: Vec<String> = loaded
        .program()
        .decls
        .iter()
        .filter_map(|decl| match decl {
            Decl::Fn(f) if f.name.name.starts_with("e_") => Some(f.qualified_name()),
            _ => None,
        })
        .collect();
    assert_eq!(names.len(), 11, "{names:?}");
    let mut wrong = Vec::new();
    for name in &names {
        match evaluator.call(&QualName::new(module.clone(), name.clone()), Vec::new()) {
            Ok(value) if value.debug_text() == "true" => {}
            Ok(value) => wrong.push(format!("{name}: {}", value.debug_text())),
            Err(e) => wrong.push(format!("{name}: {e:?}")),
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// Rows 21 and 22: every wave-A fixture through the CLI with `--target
/// all`, one `ignite test` per file. `holds.ignite` has one test per
/// "holds" verdict of 20.1 (probe p01 and the items it left out);
/// `calls.ignite`, `moves.ignite` and `rest.ignite` are the fixtures of
/// rows 11, 12 and 18. Each passes every test on both targets.
#[test]
fn fixtures_pass_on_both_targets() {
    for (name, count) in [
        ("holds.ignite", 11),
        ("calls.ignite", 6),
        ("moves.ignite", 13),
        ("rest.ignite", 7),
    ] {
        let (status, out) = both_targets(name);
        assert_eq!(status, Some(0), "{name}: {out}");
        assert_eq!(
            out.matches(&format!("test result: ok. {count} passed; 0 failed;"))
                .count(),
            2,
            "{name}: {out}"
        );
        assert!(!out.contains("FAILED"), "{name}: {out}");
    }
}

/// 20.3.2 and 20.3.5: named arguments (a UFCS receiver among them) and
/// variant payloads are evaluated into values and passed or stored after
/// the last one, so an earlier operand in a move position that is copied
/// (not moved) must still read its value from before a later operand moved
/// the same place.
#[test]
fn deferred_copies_keep_their_values_on_both_targets() {
    let (status, out) = both_targets("moves_deferred.ignite");
    assert_eq!(status, Some(0), "{out}");
    assert_eq!(
        out.matches("test result: ok. 3 passed; 0 failed;").count(),
        2,
        "{out}"
    );
}
