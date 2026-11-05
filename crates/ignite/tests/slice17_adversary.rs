//! Adversarial cases of slice 17 U1 (`docs/slice.md` section 21): programs
//! that `ignite check` accepts must build on both targets (`docs/vision.md`
//! invariant 7); a compiler panic or generated code that does not compile
//! is no diagnostic.

use crate::common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/slice17_adversary")
        .join(name)
}

fn out_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("slice17_adversary")
        .join(name)
}

fn ignite_test(name: &str, target: &str) -> Output {
    let dir = out_dir(&format!("{name}-{target}"));
    Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("test")
        .arg(fixture(name))
        .args(["--target", target, "--out"])
        .arg(&dir)
        .arg("--target-dir")
        .arg(dir.join("cargo"))
        .arg("--runtime")
        .arg(common::runner_archive())
        .output()
        .expect("run ignite test")
}

fn assert_passes(output: &Output) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("test result: ok. 1 passed; 0 failed;"),
        "stdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

/// `ignite check` accepts a module that calls `child.wait()` without
/// importing anything from `std.process`.
#[test]
fn wait_without_import_checks() {
    let output = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("check")
        .arg(fixture("wait_without_import.ignite"))
        .output()
        .expect("run ignite check");
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

/// The same module on the LLVM target, the default: no compiler panic.
#[test]
fn wait_without_import_builds_on_llvm() {
    assert_passes(&ignite_test("wait_without_import.ignite", "llvm"));
}

/// The same module on the Rust target: the generated crate compiles.
#[test]
fn wait_without_import_builds_on_rust() {
    assert_passes(&ignite_test("wait_without_import.ignite", "rust"));
}

/// The class of `wait_without_import`: `Endpoint.of`, a builtin static
/// whose signature names `std.net.Endpoint`, in a module that imports
/// nothing from `std.net`, on both targets.
#[test]
fn endpoint_without_import_builds_on_both_targets() {
    for target in ["llvm", "rust"] {
        assert_passes(&ignite_test("endpoint_without_import.ignite", target));
    }
}
