//! A program that reaches a sibling module through `module("p")`
//! (`docs/slice.md` section 14.4) on the LLVM target, through both
//! `ignite build` and `ignite test`. The sibling must be lowered and linked:
//! `ignite build` yields a client that writes the sibling fn's value over a
//! real socket, and `ignite test` runs an inline test that calls it. A
//! dropped sibling fails the link (undefined `demo.ops.greeting`) or the
//! assertion, never silently.

use std::io::Read;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use ignite::Target;

use crate::common;

const TIMEOUT: Duration = Duration::from_secs(10);

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/siblings/demo.ignite")
}

fn tmp(name: &str) -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join(name)
}

/// `ignite build --target llvm` links the sibling and the client writes the
/// sibling fn's value over a socket.
#[test]
fn llvm_build_links_a_module_sibling() {
    let out = tmp("llvm-siblings-build");
    let options = ignite::BuildOptions {
        target: Target::Llvm,
        out: Some(out.clone()),
        runtime: Some(common::runtime_archive()),
        ..Default::default()
    };
    let output = ignite::build(&fixture(), &options).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(output.executables, [out.join("bin/demo")]);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut child = Command::new(&output.executables[0])
        .args(["--port", &port.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn demo");
    let (mut stream, _) = listener.accept().unwrap();
    stream.set_read_timeout(Some(TIMEOUT)).unwrap();
    let mut wrote = Vec::new();
    stream.read_to_end(&mut wrote).unwrap();
    let status = child.wait().unwrap();
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    assert_eq!(status.code(), Some(0), "stderr: {stderr}");
    assert_eq!(wrote, b"hello");
}

/// `ignite test --target llvm` lowers the sibling into the test executable
/// and the inline test that calls the sibling fn passes.
#[test]
fn llvm_test_runs_a_module_sibling() {
    let out = tmp("llvm-siblings-test");
    let output = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("test")
        .arg(fixture())
        .args(["--target", "llvm", "--out"])
        .arg(&out)
        .arg("--runtime")
        .arg(common::runner_archive())
        .output()
        .expect("run ignite test");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("test demo/module greeting ... ok"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("test result: ok. 1 passed; 0 failed;"),
        "stdout: {stdout}"
    );
}
