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
/// reads garbage if a move left it pointing at released memory.
#[test]
fn llvm_fixtures_pass() {
    let dir = out_dir("fixtures_llvm");
    let (status, stdout) = llvm_test(&dir, &["moves.ignite", "calls.ignite"], &[]);
    assert_eq!(status, Some(0), "{stdout}");
    let results: Vec<&str> = stdout
        .lines()
        .filter(|line| line.starts_with("test result: "))
        .collect();
    assert_eq!(results.len(), 2, "one executable per file:\n{stdout}");
    assert!(
        results[0].starts_with("test result: ok. 13 passed; 0 failed;"),
        "moves.ignite:\n{stdout}"
    );
    assert!(
        results[1].starts_with("test result: ok. 6 passed; 0 failed;"),
        "calls.ignite:\n{stdout}"
    );
    assert!(!stdout.contains("FAILED"), "{stdout}");
}
