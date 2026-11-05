//! Builds the runtime archive of the LLVM target, `libignite_rt.a` with the
//! test runner (`ignite-rt --features runner`), so that the driver can carry
//! it (`src/embedded_runtime.rs`): an installed `ignite` then builds and
//! tests without `--runtime`.
//!
//! The archive is built by a nested `cargo build` of `ignite-rt` from this
//! workspace, for the target and profile of the driver, into a target
//! directory of its own beside this script's output directory, so that the
//! check and the build of the driver share it. Its path and its content
//! hash reach the crate as `IGNITE_RT_ARCHIVE` and `IGNITE_RT_HASH`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let manifest = PathBuf::from(env("CARGO_MANIFEST_DIR"));
    let crates = manifest.parent().expect("crates directory");
    let workspace = crates.parent().expect("workspace directory");
    for path in [
        crates.join("ignite-rt/src"),
        crates.join("ignite-rt/Cargo.toml"),
        crates.join("ignite-io/src"),
        crates.join("ignite-io/Cargo.toml"),
        workspace.join("Cargo.toml"),
        workspace.join("Cargo.lock"),
    ] {
        println!("cargo::rerun-if-changed={}", path.display());
    }

    let out = PathBuf::from(env("OUT_DIR"));
    let target_dir = runtime_target_dir(&out);
    let target = env("TARGET");
    let release = env("PROFILE") == "release";
    let mut command = Command::new(env("CARGO"));
    command
        .args([
            "build",
            "--locked",
            "-p",
            "ignite-rt",
            "--features",
            "runner",
        ])
        .arg("--manifest-path")
        .arg(crates.join("ignite-rt/Cargo.toml"))
        .arg("--target")
        .arg(&target)
        .arg("--target-dir")
        .arg(&target_dir)
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .env_remove("CARGO_TARGET_DIR")
        .env_remove("CARGO_BUILD_TARGET");
    if release {
        command.arg("--release");
    }
    let status = command
        .status()
        .unwrap_or_else(|error| panic!("start {command:?}: {error}"));
    assert!(status.success(), "{command:?} failed with {status}");

    let archive = target_dir
        .join(&target)
        .join(if release { "release" } else { "debug" })
        .join("libignite_rt.a");
    let bytes = std::fs::read(&archive)
        .unwrap_or_else(|error| panic!("read {}: {error}", archive.display()));
    println!("cargo::rustc-env=IGNITE_RT_ARCHIVE={}", archive.display());
    println!("cargo::rustc-env=IGNITE_RT_HASH={:016x}", fnv1a(&bytes));
}

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set"))
}

/// `<target>/<profile>/build/ignite-runtime` for an output directory
/// `<target>/<profile>/build/ignite-<hash>/out`, else `<out>/runtime`.
fn runtime_target_dir(out: &Path) -> PathBuf {
    match out.parent().and_then(Path::parent) {
        Some(build) if build.file_name().is_some_and(|name| name == "build") => {
            build.join("ignite-runtime")
        }
        _ => out.join("runtime"),
    }
}

/// FNV-1a, 64 bits: names the archive's cache directory, so that drivers
/// carrying different archives never share one.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}
