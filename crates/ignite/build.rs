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
//!
//! For a `<arch>-unknown-linux-gnu` driver whose toolchain has the Rust
//! target `<arch>-unknown-linux-musl`, the same `cargo build` also builds
//! the archive for that target, and `$OUT_DIR/musl_kit.rs` carries it with
//! the musl start files, `libc.a` and `libunwind.a` of the target's
//! `self-contained` directory: the kit `ignite build --static` links
//! (`docs/slice.md` 23.3). Without that target the file says `None`, and
//! `--static` then needs the target installed (23.4).

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The files of a musl target's `self-contained` directory a static link
/// needs beside the runtime archive.
const KIT_FILES: &[&str] = &[
    "crt1.o",
    "crti.o",
    "crtbegin.o",
    "crtend.o",
    "crtn.o",
    "libc.a",
    "libunwind.a",
];

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
    let musl = musl_self_contained(&target);
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
    if let Some((triple, _)) = &musl {
        command.arg("--target").arg(triple);
    }
    if release {
        command.arg("--release");
    }
    let status = command
        .status()
        .unwrap_or_else(|error| panic!("start {command:?}: {error}"));
    assert!(status.success(), "{command:?} failed with {status}");

    let profile = if release { "release" } else { "debug" };
    let archive = target_dir
        .join(&target)
        .join(profile)
        .join("libignite_rt.a");
    let bytes = std::fs::read(&archive)
        .unwrap_or_else(|error| panic!("read {}: {error}", archive.display()));
    println!("cargo::rustc-env=IGNITE_RT_ARCHIVE={}", archive.display());
    println!(
        "cargo::rustc-env=IGNITE_RT_HASH={:016x}",
        fnv1a(FNV_OFFSET, &bytes)
    );

    let kit = match &musl {
        Some((triple, self_contained)) => {
            let archive = target_dir.join(triple).join(profile).join("libignite_rt.a");
            let mut files = vec![("libignite_rt.a", archive)];
            files.extend(
                KIT_FILES
                    .iter()
                    .map(|name| (*name, self_contained.join(name))),
            );
            let mut hash = FNV_OFFSET;
            let mut entries = String::new();
            for (name, path) in &files {
                println!("cargo::rerun-if-changed={}", path.display());
                let bytes = std::fs::read(path)
                    .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
                hash = fnv1a(fnv1a(hash, name.as_bytes()), &bytes);
                let path = path.to_str().expect("a UTF-8 path");
                let _ = write!(entries, "({name:?}, include_bytes!({path:?})), ");
            }
            format!(
                "Some(Kit {{ triple: {triple:?}, hash: \"{hash:016x}\", files: &[{entries}] }})"
            )
        }
        None => "None".to_string(),
    };
    let kit_file = out.join("musl_kit.rs");
    std::fs::write(
        &kit_file,
        format!("static MUSL_KIT: Option<Kit> = {kit};\n"),
    )
    .unwrap_or_else(|error| panic!("write {}: {error}", kit_file.display()));
}

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set"))
}

/// For a target `<arch>-unknown-linux-gnu`, the triple
/// `<arch>-unknown-linux-musl` and its `self-contained` directory, when the
/// toolchain has that target with every file of [`KIT_FILES`].
fn musl_self_contained(target: &str) -> Option<(String, PathBuf)> {
    let triple = format!("{}-linux-musl", target.strip_suffix("-linux-gnu")?);
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let output = Command::new(rustc)
        .args(["--print", "target-libdir", "--target", &triple])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let libdir = PathBuf::from(String::from_utf8(output.stdout).ok()?.trim());
    let has_std = std::fs::read_dir(&libdir).ok()?.any(|entry| {
        entry.is_ok_and(|entry| entry.file_name().to_string_lossy().starts_with("libstd-"))
    });
    let self_contained = libdir.join("self-contained");
    (has_std
        && KIT_FILES
            .iter()
            .all(|name| self_contained.join(name).is_file()))
    .then_some((triple, self_contained))
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

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

/// FNV-1a, 64 bits, continuing from `hash` ([`FNV_OFFSET`] starts a new
/// one): names the archive's cache directory, so that drivers carrying
/// different archives never share one.
fn fnv1a(mut hash: u64, bytes: &[u8]) -> u64 {
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}
