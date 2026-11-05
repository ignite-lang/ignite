//! Hands the runtime archive of the LLVM target, `libignite_rt.a` with the
//! test runner (`ignite-rt --features runner`), to the driver, which carries
//! it (`src/embedded_runtime.rs`): an installed `ignite` then builds and
//! tests without `--runtime`.
//!
//! The archive is the staticlib of the build-dependency `ignite-rt`, which
//! the build that runs this script has compiled already, into its own
//! target directory: cargo compiles every build-dependency before the
//! script, and `ignite-rt` declares `crate-type = ["staticlib", "rlib"]`.
//! Build-dependencies resolve their features apart from the normal ones, so
//! the `panic-probe` of `ignite-llvm`'s dev-dependency never reaches it. The
//! workspace's `build-override` profiles match `dev` and `release`, so the
//! substrate units shared with the normal graph are compiled once.
//!
//! Cargo names the archive `deps/libignite_rt-<hash>.a` beside this
//! script's build directory and tells the script no hash, so the script
//! takes the newest archive there that carries the runner, has no probe and
//! is not newer than the script itself. A cross build (`TARGET` other than
//! `HOST`) compiles build-dependencies for the host, so it builds the
//! archive for `TARGET` by a nested `cargo build` into a target directory
//! of its own instead. Its path and its content hash reach the crate as
//! `IGNITE_RT_ARCHIVE` and `IGNITE_RT_HASH`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

/// `ignite_rt::PANIC_PROBE`, present only with the feature `panic-probe`.
const PANIC_PROBE: &[u8] = b"ignite-rt panic probe";

/// The test runner's entry, present only with the feature `runner`.
const RUNNER: &[u8] = b"ignite_rt_test_main";

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    let target = env("TARGET");
    let (archive, bytes) = if target == env("HOST") {
        built_dependency()
    } else {
        cross(&target)
    };
    println!("cargo::rustc-env=IGNITE_RT_ARCHIVE={}", archive.display());
    println!("cargo::rustc-env=IGNITE_RT_HASH={:016x}", fnv1a(&bytes));
}

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set"))
}

/// The staticlib of the build-dependency `ignite-rt`:
/// `<target>/<profile>/deps/libignite_rt-<hash>.a` for this script at
/// `<target>/<profile>/build/ignite-<hash>/build-script-build`.
fn built_dependency() -> (PathBuf, Vec<u8>) {
    let script = std::env::current_exe().expect("build script path");
    let built = modified(&script).expect("build script modification time");
    let deps = script
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .expect("build script under <target>/<profile>/build/<unit>")
        .join("deps");
    let mut archives: Vec<(SystemTime, PathBuf)> = std::fs::read_dir(&deps)
        .unwrap_or_else(|error| panic!("read {}: {error}", deps.display()))
        .filter_map(Result::ok)
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.starts_with("libignite_rt-") && name.ends_with(".a")
        })
        .filter_map(|entry| Some((modified(&entry.path())?, entry.path())))
        .filter(|(time, _)| *time <= built)
        .collect();
    archives.sort();
    archives
        .into_iter()
        .rev()
        .find_map(|(_, path)| {
            let bytes = std::fs::read(&path).ok()?;
            let own = own_objects(&bytes)?;
            (own.iter().any(|object| contains(object, RUNNER))
                && !own.iter().any(|object| contains(object, PANIC_PROBE)))
            .then_some((path, bytes))
        })
        .unwrap_or_else(|| {
            panic!(
                "no libignite_rt-*.a with the runner and without the panic probe in {}",
                deps.display()
            )
        })
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

/// The members of the (GNU) archive `bytes` that rustc compiled from
/// `ignite-rt` itself, `ignite_rt-<hash>.ignite_rt.<cgu>.rcgu.o`: a few
/// megabytes at its start, before the bundled `ignite-io`, `nix` and `std`.
/// `None` if `bytes` is no such archive.
fn own_objects(bytes: &[u8]) -> Option<Vec<&[u8]>> {
    let mut rest = bytes.strip_prefix(b"!<arch>\n")?;
    let mut names: &[u8] = &[];
    let mut own = Vec::new();
    while rest.len() >= 60 {
        let (header, body) = rest.split_at(60);
        let size: usize = std::str::from_utf8(&header[48..58])
            .ok()?
            .trim()
            .parse()
            .ok()?;
        let data = body.get(..size)?;
        rest = body.get(size + size % 2..).unwrap_or(&[]);
        let name = &header[..16];
        if name.starts_with(b"// ") {
            names = data;
        } else if let Some(offset) = name.strip_prefix(b"/") {
            let offset = std::str::from_utf8(offset).ok()?.trim();
            let Ok(offset) = offset.parse::<usize>() else {
                continue;
            };
            if names.get(offset..)?.starts_with(b"ignite_rt-") {
                own.push(data);
            }
        } else if name.starts_with(b"ignite_rt-") {
            own.push(data);
        }
    }
    Some(own)
}

fn contains(bytes: &[u8], needle: &[u8]) -> bool {
    bytes.windows(needle.len()).any(|window| window == needle)
}

/// A nested `cargo build` of `ignite-rt --features runner` for `target` and
/// the driver's profile, into `<target>/<profile>/build/ignite-runtime`.
fn cross(target: &str) -> (PathBuf, Vec<u8>) {
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
        .arg(target)
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
        .join(target)
        .join(if release { "release" } else { "debug" })
        .join("libignite_rt.a");
    let bytes = std::fs::read(&archive)
        .unwrap_or_else(|error| panic!("read {}: {error}", archive.display()));
    (archive, bytes)
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
