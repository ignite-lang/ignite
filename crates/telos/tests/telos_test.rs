//! `telos test` end to end on both targets (`docs/slice.md` section 12):
//! the fixtures in `tests/fixtures/tests` are built into test executables
//! and run; the report on stdout and the exit status are checked.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

/// One `telos test` at a time: invocations share the output directory of
/// their target and cargo's target directory.
static BUILDS: Mutex<()> = Mutex::new(());

const TARGETS: [&str; 2] = ["rust", "llvm"];

fn fixture(name: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/tests")
        .join(name)
        .display()
        .to_string()
}

fn out_dir() -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join("telos-test")
}

struct Run {
    status: i32,
    stdout: String,
    stderr: String,
}

/// `telos test <args> --target <target>`.
fn telos_test(target: &str, args: &[&str]) -> Run {
    let _guard = BUILDS.lock().unwrap_or_else(|e| e.into_inner());
    let output = Command::new(env!("CARGO_BIN_EXE_telos"))
        .arg("test")
        .args(args)
        .args(["--target", target, "--out"])
        .arg(out_dir())
        .arg("--runtime")
        .arg(runner_runtime())
        .arg("--target-dir")
        .arg(out_dir().join("cargo"))
        .output()
        .expect("run telos test");
    Run {
        status: output.status.code().expect("telos test exits"),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// The report without the time of the summary line.
fn without_time(report: &str) -> String {
    report
        .lines()
        .map(|line| match line.find("; finished in ") {
            Some(at) => &line[..at],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn basics(target: &str) -> Run {
    telos_test(target, &[&fixture("basics.telos")])
}

#[test]
fn counts_results_and_failures() {
    for target in TARGETS {
        let run = basics(target);
        let out = &run.stdout;
        assert_eq!(run.status, 1, "{target}: {out}\n{}", run.stderr);
        assert!(out.starts_with("running 12 tests\n"), "{target}: {out}");
        for line in [
            "test fixtures.basics/arithmetic passes ... ok",
            "test fixtures.basics/int differs ... FAILED",
            "test fixtures.basics/exit code 3 ... FAILED (exited with code 3)",
            "test fixtures.basics/index out of range panics ... FAILED",
            "test fixtures.basics/output hidden on success ... ok",
            "test fixtures.basics/tcp echo through a task ... ok",
            "test result: FAILED. 3 passed; 9 failed; 0 filtered out; finished in ",
        ] {
            assert!(out.contains(line), "{target}: no `{line}` in\n{out}");
        }
        assert!(
            out.contains("failures:\n    fixtures.basics/int differs\n"),
            "{target}: {out}"
        );
    }
}

#[test]
fn assert_and_try_reports() {
    let path = fixture("basics.telos");
    for target in TARGETS {
        let out = basics(target).stdout;
        for report in [
            format!(
                "---- fixtures.basics/int differs stderr ----\nassertion failed: 1 + 1 == 3\n  left: 2\n  right: 3\n  at {path}:33:3\n"
            ),
            format!(
                "assertion failed: \" ab \".trim() == \"ab \"\n  left: ab\n  right: ab \n  at {path}:37:3\n"
            ),
            format!(
                "assertion failed: Point {{ x = 1, y = 2 }} == Point {{ x = 1, y = 3 }}\n  left: (1, 2)\n  right: (1, 3)\n  at {path}:41:3\n"
            ),
            format!("assertion failed: n < 3\n  message: n is 4\n  at {path}:46:3\n"),
            format!("`?` failed on Err: not a number: x\n  at {path}:50:21\n"),
            format!("`?` failed on None\n  at {path}:55:25\n"),
            "---- fixtures.basics/index out of range panics stderr ----\npanic: index out of "
                .to_string(),
        ] {
            assert!(out.contains(&report), "{target}: no\n{report}\nin\n{out}");
        }
    }
}

#[test]
fn captured_output_only_for_failures() {
    for target in TARGETS {
        let out = basics(target).stdout;
        assert!(
            out.contains(
                "---- fixtures.basics/output shown on failure stdout ----\ncaptured out\n"
            ),
            "{target}: {out}"
        );
        assert!(!out.contains("hidden out"), "{target}: {out}");
        assert!(
            !out.contains("---- fixtures.basics/exit code 3 stdout"),
            "{target}: {out}"
        );
    }
}

#[test]
fn both_targets_print_the_same_report() {
    let reports: Vec<String> = TARGETS
        .iter()
        .map(|target| {
            without_time(&basics(target).stdout)
                .lines()
                .filter(|line| !line.starts_with("panic: "))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect();
    assert_eq!(reports[0], reports[1]);
}

#[test]
fn filter_exact_and_list() {
    let file = fixture("basics.telos");
    for target in TARGETS {
        let list = telos_test(target, &[&file, "differs", "--list"]);
        assert_eq!(list.status, 0, "{target}: {}", list.stderr);
        assert_eq!(
            list.stdout,
            "fixtures.basics/int differs: test\nfixtures.basics/text differs: test\nfixtures.basics/record differs: test\n3 tests\n",
            "{target}"
        );
        let exact = telos_test(target, &[&file, "fixtures.basics/int differs", "--exact"]);
        assert_eq!(exact.status, 1, "{target}: {}", exact.stdout);
        assert!(exact.stdout.starts_with("running 1 test\n"), "{target}");
        assert!(
            exact
                .stdout
                .contains("test result: FAILED. 0 passed; 1 failed; 11 filtered out;"),
            "{target}: {}",
            exact.stdout
        );
        let passing = telos_test(target, &[&file, "passes"]);
        assert_eq!(passing.status, 0, "{target}: {}", passing.stdout);
        assert_eq!(
            without_time(&passing.stdout),
            "running 1 test\ntest fixtures.basics/arithmetic passes ... ok\n\ntest result: ok. 1 passed; 0 failed; 11 filtered out",
            "{target}"
        );
        let none = telos_test(target, &[&file, "no such test"]);
        assert_eq!(none.status, 0, "{target}");
        assert_eq!(
            without_time(&none.stdout),
            "running 0 tests\n\ntest result: ok. 0 passed; 0 failed; 12 filtered out",
            "{target}"
        );
    }
}

#[test]
fn each_file_runs_its_own_tests() {
    let (basics, slow) = (fixture("basics.telos"), fixture("slow.telos"));
    for target in TARGETS {
        let list = telos_test(target, &[&basics, &slow, "fa", "--list"]);
        assert_eq!(list.status, 0, "{target}: {}", list.stderr);
        assert_eq!(
            list.stdout,
            "fixtures.basics/try on err fails: test\nfixtures.basics/try on none fails: test\nfixtures.basics/output shown on failure: test\n3 tests\nfixtures.slow/fast: test\n1 test\n",
            "{target}"
        );
    }
}

#[test]
fn timeout_kills_a_test() {
    for target in TARGETS {
        let run = telos_test(target, &[&fixture("slow.telos"), "--timeout", "1"]);
        assert_eq!(run.status, 1, "{target}: {}\n{}", run.stdout, run.stderr);
        assert!(
            run.stdout
                .contains("test fixtures.slow/never ends ... FAILED (timed out after 1 s)\n"),
            "{target}: {}",
            run.stdout
        );
        assert!(
            run.stdout
                .contains("test result: FAILED. 1 passed; 1 failed; 0 filtered out;"),
            "{target}: {}",
            run.stdout
        );
    }
}

#[test]
fn compile_error_exits_2() {
    for target in TARGETS {
        let run = telos_test(target, &[&fixture("broken.telos")]);
        assert_eq!(run.status, 2, "{target}: {}", run.stderr);
        assert!(
            run.stderr.contains("error[E040]"),
            "{target}: {}",
            run.stderr
        );
        assert!(run.stdout.is_empty(), "{target}: {}", run.stdout);
    }
}

#[test]
fn std_module_as_the_file() {
    let std = fixture("std");
    for target in TARGETS {
        let run = telos_test(target, &[&fixture("std/greet.telos"), "--std", &std]);
        assert_eq!(run.status, 0, "{target}: {}\n{}", run.stdout, run.stderr);
        assert_eq!(
            without_time(&run.stdout),
            "running 1 test\ntest std.greet/greets by name ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 filtered out",
            "{target}"
        );
    }
}

/// The newest `libtelos_rt-*.a` beside the test executable that has the
/// test runner. This crate depends on `telos-rt` with the feature
/// `runner`, but a `cargo build -p telos-rt` into the same target
/// directory leaves a newer archive without it.
fn runner_runtime() -> PathBuf {
    let exe = std::env::current_exe().expect("test executable path");
    let deps = exe.parent().expect("test executable directory");
    let mut archives: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(deps)
        .expect("read the deps directory")
        .filter_map(Result::ok)
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.starts_with("libtelos_rt-") && name.ends_with(".a")
        })
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .collect();
    archives.sort();
    let symbol = b"telos_rt_test_main";
    archives
        .into_iter()
        .rev()
        .map(|(_, path)| path)
        .find(|path| {
            std::fs::read(path)
                .is_ok_and(|bytes| bytes.windows(symbol.len()).any(|window| window == symbol))
        })
        .unwrap_or_else(common::runtime_archive)
}
