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
