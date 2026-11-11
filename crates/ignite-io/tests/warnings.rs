//! The library as a compiled program builds it: without tests and without
//! the `tls` feature (the Rust target's build of `terminald`). Code only
//! tests or TLS reach is gated to them, so that build prints no warning.

use std::path::Path;
use std::process::Command;

#[test]
fn the_library_alone_builds_without_a_warning() {
    let output = Command::new(env!("CARGO"))
        .args([
            "check",
            "--lib",
            "--message-format",
            "short",
            "--manifest-path",
        ])
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .arg("--target-dir")
        .arg(Path::new(env!("CARGO_TARGET_TMPDIR")).join("library-alone"))
        .output()
        .expect("cargo runs");
    let printed = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{printed}");
    assert!(!printed.contains("warning"), "{printed}");
}
