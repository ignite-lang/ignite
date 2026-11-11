//! Adversary case of `story:binary-size-back`: the generated project's dev
//! profile builds `ignite-std` without debug information
//! (`STD_DEV_PROFILE` in `crates/ignite-rustgen/src/lib.rs`), and the
//! story's acceptance keeps "a dev backtrace of a failing Ignite program
//! still names file and line". `dev_build`'s backtrace fixture fails in
//! the program's own fn; this one fails in a fn of the standard library.

use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// `gdb` runs `executable` to the failure report of `ignite-io` and prints
/// the backtrace there, as `dev_build::gdb_backtrace`.
fn gdb_backtrace(executable: &Path) -> String {
    let output = Command::new("gdb")
        .args(["-nx", "-batch"])
        .args(["-ex", "set breakpoint pending on"])
        .args(["-ex", "break ignite_io::report::failure_report"])
        .args(["-ex", "run", "-ex", "bt", "-ex", "continue"])
        .arg("--args")
        .arg(executable)
        .env("DEBUGINFOD_URLS", "")
        .output()
        .expect("start gdb");
    format!("{}{}", text(&output.stdout), text(&output.stderr))
}

/// A program that fails inside the standard library (`std.time`'s
/// `days_from_civil` multiplies past the range of `Int`), built in dev
/// for the Rust target: the backtrace under `gdb` has the frame of
/// `ignite_std::time::days_from_civil`, and that frame names a file and a
/// line, as it did before the generated `ignite-std` lost its line tables
/// (`std/src/lib.rs:<line>`), and as the program's own frame beside it
/// still does.
#[test]
fn a_rust_dev_backtrace_names_file_and_line_of_a_standard_library_frame() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/backtrace_std.ignite");
    let out = tmp("dev-backtrace-std");
    let target = Path::new(env!("CARGO_TARGET_TMPDIR")).join("dev-backtrace-std-cargo");
    let build = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("build")
        .arg(&source)
        .args(["--target", "rust"])
        .arg("--out")
        .arg(&out)
        .arg("--target-dir")
        .arg(&target)
        .output()
        .expect("start ignite build");
    assert!(
        build.status.success(),
        "ignite build: {}{}",
        text(&build.stdout),
        text(&build.stderr)
    );
    let backtrace = gdb_backtrace(&target.join("debug/civil"));
    let frames: Vec<&str> = backtrace
        .lines()
        .filter(|line| line.starts_with('#'))
        .collect();
    let program = frames
        .iter()
        .find(|line| line.contains("Civil__main"))
        .unwrap_or_else(|| panic!("no frame of the program's main:\n{backtrace}"));
    assert!(
        program.contains(" at src/lib.rs:"),
        "the program's frame names no file and line: {program}\n{backtrace}"
    );
    let library = frames
        .iter()
        .find(|line| line.contains("ignite_std::time::days_from_civil"))
        .unwrap_or_else(|| panic!("no frame of days_from_civil:\n{backtrace}"));
    let named = library.rsplit_once(" at ").is_some_and(|(_, at)| {
        at.rsplit_once(':')
            .is_some_and(|(file, line)| !file.is_empty() && line.trim().parse::<usize>().is_ok())
    });
    assert!(
        named,
        "the standard library's frame names no file and line: {library}\n{}",
        frames.join("\n")
    );
}
