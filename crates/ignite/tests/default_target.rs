//! The default target is LLVM (`docs/vision.md` section 8, D2 B; X25):
//! every command that takes `--target` builds with clang and the runtime
//! archive when none is given, and `--target rust` keeps the Rust target.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crate::common;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn ignite(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ignite"))
        .args(args)
        .current_dir(workspace())
        .output()
        .expect("run ignite")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("default-target-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// `ignite build examples/todo/todo.ignite` without `--target` links `todod`
/// with clang: the IR and the executable under `<out>`, no Cargo project.
#[test]
fn build_without_target_writes_an_llvm_built_todod() {
    let out = tmp("todo");
    let runtime = common::runtime_archive();
    let output = ignite(&[
        "build",
        "examples/todo/todo.ignite",
        "--out",
        out.to_str().unwrap(),
        "--runtime",
        runtime.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let todod = out.join("bin").join("todod");
    assert_eq!(text(&output.stdout), format!("built {}\n", todod.display()));
    assert!(todod.is_file(), "{}", todod.display());
    assert!(out.join("llvm").join("program.ll").is_file());
    assert!(out.join("llvm").join("std.ll").is_file());
    assert!(!out.join("Cargo.toml").exists());
    assert!(!out.join("std").join("Cargo.toml").exists());
}

/// The library's defaults agree with the CLI's.
#[test]
fn library_options_default_to_llvm() {
    assert_eq!(ignite::Target::default(), ignite::Target::Llvm);
    assert_eq!(ignite::BuildOptions::default().target, ignite::Target::Llvm);
    assert_eq!(ignite::RunOptions::default().target, ignite::Target::Llvm);
}

/// Every command that takes `--target` names `llvm` as its default.
#[test]
fn help_names_llvm_as_the_default_target() {
    for command in ["build", "test", "run", "plan", "conformance"] {
        let output = ignite(&[command, "--help"]);
        assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
        let help = text(&output.stdout);
        assert!(help.contains("[default: llvm]"), "{command}: {help}");
        assert!(!help.contains("[default: rust]"), "{command}: {help}");
    }
}

/// A missing runtime archive names both ways out: build the archive, or
/// choose the Rust target.
#[test]
fn a_missing_runtime_names_the_fix() {
    let out = tmp("missing-runtime");
    let runtime = out.join("nowhere").join("libignite_rt.a");
    let output = ignite(&[
        "build",
        "examples/todo/todo.ignite",
        "--out",
        out.to_str().unwrap(),
        "--runtime",
        runtime.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(3), "{}", text(&output.stderr));
    let stderr = text(&output.stderr);
    assert!(stderr.contains(runtime.to_str().unwrap()), "{stderr}");
    assert!(stderr.contains("cargo build -p ignite-rt"), "{stderr}");
    assert!(stderr.contains("--runtime"), "{stderr}");
    assert!(stderr.contains("--target rust"), "{stderr}");
}

/// `ignite test` without `--target` links the runner archive; a missing one
/// names the feature that adds the runner.
#[test]
fn test_without_target_names_the_runner_runtime_when_it_is_missing() {
    let out = tmp("missing-runner");
    let runtime = out.join("nowhere").join("libignite_rt.a");
    let output = ignite(&[
        "test",
        "std/option.ignite",
        "--out",
        out.to_str().unwrap(),
        "--runtime",
        runtime.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(3), "{}", text(&output.stderr));
    let stderr = text(&output.stderr);
    assert!(stderr.contains(runtime.to_str().unwrap()), "{stderr}");
    assert!(
        stderr.contains("cargo build -p ignite-rt --features runner"),
        "{stderr}"
    );
    assert!(stderr.contains("--target rust"), "{stderr}");
}
