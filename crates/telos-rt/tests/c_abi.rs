//! The C ABI links outside cargo: `tests/smoke.c` is compiled and linked
//! with `clang` against the built `libtelos_rt.a` and the native libraries
//! rustc names for the staticlib, then run.
//!
//! Where the archive is: cargo builds the library's `staticlib` and `rlib`
//! in one rustc run before it builds this test, into the `deps` directory
//! this test binary lives in, as `libtelos_rt-<hash>.a`. As a dependency of
//! a test it is not uplifted, so `target/debug/libtelos_rt.a` exists only
//! after a `cargo build` and can be stale. The newest `libtelos_rt-*.a` in
//! `deps` is the one built from the current source.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;

/// `cargo rustc -p telos-rt --crate-type staticlib -- --print
/// native-static-libs` on x86_64-unknown-linux-gnu.
const NATIVE_LIBS: &[&str] = &[
    "-lgcc_s",
    "-lutil",
    "-lrt",
    "-lpthread",
    "-lm",
    "-ldl",
    "-lc",
];

fn archive() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    let deps = exe.parent().unwrap();
    std::fs::read_dir(deps)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.starts_with("libtelos_rt-") && name.ends_with(".a")
        })
        .max_by_key(|entry| entry.metadata().and_then(|m| m.modified()).ok())
        .map(|entry| entry.path())
        .unwrap_or_else(|| panic!("no libtelos_rt-*.a in {}", deps.display()))
}

fn smoke() -> &'static Path {
    static BINARY: OnceLock<PathBuf> = OnceLock::new();
    BINARY.get_or_init(|| {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/smoke.c");
        let binary = Path::new(env!("CARGO_TARGET_TMPDIR")).join("telos-rt-smoke");
        let output = Command::new("clang")
            .arg("-Wall")
            .arg("-Werror")
            .arg("-o")
            .arg(&binary)
            .arg(&source)
            .arg(archive())
            .args(NATIVE_LIBS)
            .output()
            .expect("clang on PATH");
        assert!(
            output.status.success(),
            "clang failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        binary
    })
}

fn run(args: &[&str]) -> Output {
    Command::new(smoke()).args(args).output().unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn smoke_program_runs() {
    let output = run(&["smoke"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(
        text(&output.stdout),
        "layout 24 32 8 32\npadded\ntelos\nx-y-z\n-1234567890123\na\u{fffd}\neq 1 starts 1\necho: ping\n"
    );
    assert_eq!(text(&output.stderr), "logged to stderr\n");
}

#[test]
fn port_argument() {
    let output = run(&["port", "--port", "4242"]);
    assert_eq!(text(&output.stdout), "port 4242 present 1\n");
    let output = run(&["port"]);
    assert_eq!(text(&output.stdout), "port 0 present 0\n");
    let output = run(&["port", "--port", "abc"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        text(&output.stderr),
        "error: --port abc: expected an integer in 0..=65535\n"
    );
}

#[test]
fn fail_panic_and_exit_statuses() {
    let output = run(&["fail"]);
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(text(&output.stderr), "error: boom\n");
    let output = run(&["panic"]);
    assert_eq!(output.status.code(), Some(101));
    assert_eq!(text(&output.stderr), "panic: index 9 out of range\n");
    let output = run(&["exit"]);
    assert_eq!(output.status.code(), Some(7));
}
