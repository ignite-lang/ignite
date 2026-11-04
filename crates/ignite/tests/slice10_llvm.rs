//! The slice-10 fixtures of `crates/ignite-rustgen/tests/slice10` on both
//! targets through `ignite test`: `features.ignite` (quantities, named
//! arguments, brace literals, `Map`, `with_exit_code`; `docs/slice.md` 14.1
//! to 14.3) passes on both, and `failures.ignite` (section 14.8: slice, at,
//! div, quantity overflow, neg) fails on both, the LLVM target printing the
//! same report as the Rust target byte for byte.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use crate::common;

/// One `ignite test` at a time: both targets share cargo's target directory.
static BUILDS: Mutex<()> = Mutex::new(());

const TARGETS: [&str; 2] = ["rust", "llvm"];

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../ignite-rustgen/tests/slice10")
        .join(name)
}

fn out_dir() -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join("slice10-llvm")
}

struct Run {
    status: i32,
    stdout: String,
    stderr: String,
}

/// `ignite test <file> --target <target>`.
fn ignite_test(target: &str, file: &Path) -> Run {
    let _guard = BUILDS.lock().unwrap_or_else(|e| e.into_inner());
    let output = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("test")
        .arg(file)
        .args(["--target", target, "--out"])
        .arg(out_dir())
        .arg("--runtime")
        .arg(common::runner_archive())
        .arg("--target-dir")
        .arg(out_dir().join("cargo"))
        .output()
        .expect("run ignite test");
    Run {
        status: output.status.code().expect("ignite test exits"),
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

#[test]
fn features_pass_on_both_targets_with_the_same_report() {
    let file = fixture("features.ignite");
    let mut reports = Vec::new();
    for target in TARGETS {
        let run = ignite_test(target, &file);
        assert_eq!(run.status, 0, "{target}:\n{}\n{}", run.stdout, run.stderr);
        assert!(
            run.stdout.contains("test result: ok. 9 passed; 0 failed;"),
            "{target}: {}",
            run.stdout
        );
        reports.push(without_time(&run.stdout));
    }
    assert_eq!(reports[0], reports[1]);
}

#[test]
fn failures_fail_on_both_targets_with_the_same_report() {
    let file = fixture("failures.ignite");
    let mut reports = Vec::new();
    for target in TARGETS {
        let run = ignite_test(target, &file);
        assert_eq!(run.status, 1, "{target}:\n{}\n{}", run.stdout, run.stderr);
        for (op, detail) in [
            ("slice", "slice 1..5 out of range for length 3"),
            ("index", "index 3 out of range for length 1"),
            ("/", "1 / 0"),
            ("+", "9223372036854775807 + 1"),
            ("-", "-(-9223372036854775808)"),
            ("slice", "slice 1..3 out of range for length 2"),
            ("slice", "slice -1..1 out of range for length 2"),
        ] {
            assert!(
                run.stdout.contains(&format!("error: {op} failed at "))
                    && run.stdout.contains(&format!(": {detail}\n")),
                "{target}: no `{op} … {detail}` in\n{}",
                run.stdout
            );
        }
        assert!(
            !run.stdout.contains("error: error"),
            "{target}: {}",
            run.stdout
        );
        reports.push(without_time(&run.stdout));
    }
    assert_eq!(reports[0], reports[1]);
}
