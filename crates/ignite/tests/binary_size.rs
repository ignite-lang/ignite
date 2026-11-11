//! Story `binary-size-back` (rows 3.2, 3.8 and 3.9 of `docs/quality.md`):
//! what the links of both targets write into an executable, read from the
//! executable itself.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crate::common;

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn run(command: &mut Command) -> Output {
    let output = command.output().expect("start the command");
    assert!(
        output.status.success(),
        "{command:?}\nstdout: {}\nstderr: {}",
        text(&output.stdout),
        text(&output.stderr)
    );
    output
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/backtrace.ignite")
}

/// `ignite build <fixture>` on `target` into `out`, dev or release; the
/// path of its executable `overflow`.
fn build(target: &str, release: bool, out: &Path, cargo_target: &Path) -> PathBuf {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ignite"));
    command
        .arg("build")
        .arg(fixture())
        .args(["--target", target])
        .arg("--out")
        .arg(out);
    if target == "llvm" {
        command.arg("--runtime").arg(common::runtime_archive());
    } else {
        command.arg("--target-dir").arg(cargo_target);
    }
    if release {
        command.arg("--release");
    }
    run(&mut command);
    match (target, release) {
        ("llvm", _) => out.join("bin/overflow"),
        (_, true) => cargo_target.join("release/overflow"),
        (_, false) => cargo_target.join("debug/overflow"),
    }
}

/// `readelf <flag> <file>`.
fn readelf(flag: &str, file: &Path) -> String {
    text(&run(Command::new("readelf").arg(flag).arg(file)).stdout)
}

/// Row 3.9 and 3.2: an executable of either target, dev and release,
/// keeps its relative relocations packed (`DT_RELR`, the linker's
/// `-z pack-relative-relocs`) and names the loader version that reads
/// them (`GLIBC_ABI_DT_RELR`, glibc 2.36), so a loader without it refuses
/// the executable instead of running it unrelocated.
#[test]
fn executables_pack_their_relative_relocations_on_both_targets() {
    let dir = tmp("size-relr");
    let cargo_target = dir.join("cargo");
    for target in ["llvm", "rust"] {
        for release in [false, true] {
            let profile = if release { "release" } else { "dev" };
            let out = dir.join(format!("{target}-{profile}"));
            let executable = build(target, release, &out, &cargo_target);
            let dynamic = readelf("-dW", &executable);
            assert!(
                dynamic.contains("(RELR)"),
                "{target} {profile}: {} has no DT_RELR:\n{dynamic}",
                executable.display()
            );
            let versions = readelf("-VW", &executable);
            assert!(
                versions.contains("GLIBC_ABI_DT_RELR"),
                "{target} {profile}: {} needs no GLIBC_ABI_DT_RELR:\n{versions}",
                executable.display()
            );
        }
    }
}
