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
//!
//! For a `-linux-gnu` driver a second `cargo build`, run beside the first
//! into `ignite-runtime-lto`, builds the same archive with
//! `-Clinker-plugin-lto`: its Rust code is LLVM bitcode, which
//! `ignite build --release` links with LTO (`docs/llvm.md` section 1).
//! `$OUT_DIR/lto_runtime.rs` carries it with the triple and the major LLVM
//! version of the rustc that built it, which the driver compares with
//! clang's; when that build fails the file says `None` and release builds
//! link the native archive.
//!
//! It also sets `IGNITE_VERSION`, the version `ignite --version` prints
//! (`src/version.rs`): the package version, with the
//! commit the driver was built from unless it is a clean build of the
//! release tag.

#[path = "src/version/describe.rs"]
mod version;

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
    version(workspace);

    let out = PathBuf::from(env("OUT_DIR"));
    let target_dir = runtime_target_dir(&out);
    let target = env("TARGET");
    let release = env("PROFILE") == "release";
    let musl = musl_self_contained(&target);
    let runtime_build = |target_dir: &Path| {
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
            .arg(target_dir)
            .env_remove("RUSTC_WORKSPACE_WRAPPER")
            .env_remove("CARGO_TARGET_DIR")
            .env_remove("CARGO_BUILD_TARGET");
        if release {
            command.arg("--release");
        }
        command
    };
    let mut command = runtime_build(&target_dir);
    if let Some((triple, _)) = &musl {
        command.arg("--target").arg(triple);
    }
    let lto_dir = target_dir.with_file_name("ignite-runtime-lto");
    let llvm = rustc_llvm_major();
    let lto_build = match llvm {
        Some(_) if target.ends_with("-linux-gnu") => {
            let mut flags = std::env::var("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default();
            if !flags.is_empty() {
                flags.push('\u{1f}');
            }
            flags.push_str("-Clinker-plugin-lto");
            let mut lto = runtime_build(&lto_dir);
            lto.env("CARGO_ENCODED_RUSTFLAGS", flags)
                .env_remove("RUSTFLAGS");
            Some(
                lto.spawn()
                    .map(|child| (lto, child))
                    .unwrap_or_else(|error| panic!("start the bitcode runtime build: {error}")),
            )
        }
        _ => None,
    };
    let status = command
        .status()
        .unwrap_or_else(|error| panic!("start {command:?}: {error}"));
    assert!(status.success(), "{command:?} failed with {status}");

    let profile = if release { "release" } else { "debug" };
    let lto = match (lto_build, llvm) {
        (Some((command, mut child)), Some(major)) => {
            let status = child
                .wait()
                .unwrap_or_else(|error| panic!("wait for {command:?}: {error}"));
            if status.success() {
                let archive = lto_dir.join(&target).join(profile).join("libignite_rt.a");
                let bytes = std::fs::read(&archive)
                    .unwrap_or_else(|error| panic!("read {}: {error}", archive.display()));
                let path = archive.to_str().expect("a UTF-8 path");
                format!(
                    "Some(LtoRuntime {{ triple: {target:?}, llvm_major: {major}, hash: \"{:016x}\", archive: include_bytes!({path:?}) }})",
                    fnv1a(FNV_OFFSET, &bytes)
                )
            } else {
                println!(
                    "cargo::warning=the bitcode runtime did not build ({status}); release builds link without LTO"
                );
                "None".to_string()
            }
        }
        _ => "None".to_string(),
    };
    let lto_file = out.join("lto_runtime.rs");
    std::fs::write(
        &lto_file,
        format!("static LTO_RUNTIME: Option<LtoRuntime> = {lto};\n"),
    )
    .unwrap_or_else(|error| panic!("write {}: {error}", lto_file.display()));

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

/// The major version of the LLVM the compiling rustc uses (`rustc -vV`,
/// `LLVM version: X.Y.Z`).
fn rustc_llvm_major() -> Option<u32> {
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let output = Command::new(rustc).arg("-vV").output().ok()?;
    let text = String::from_utf8(output.stdout).ok()?;
    let version = text
        .lines()
        .find_map(|line| line.strip_prefix("LLVM version: "))?;
    version.split('.').next()?.trim().parse().ok()
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

/// Sets `IGNITE_VERSION`, what `ignite --version` prints after `ignite `
/// (`src/version/describe.rs`): the commit of the git checkout that is the
/// workspace, else the one its `COMMIT` file records (written beside the
/// source of an archive, `dist/toolchain/Dockerfile`), else `+unknown`.
///
/// Reruns when a file of [`version::SOURCES`] changes (they decide
/// `.dirty`), when `HEAD` moves (see [`git_source`]) and when
/// `IGNITE_RELEASE_COMMIT` changes, which `task dist` sets to the commit of
/// the release tag so that a tag made after the last build is seen.
fn version(workspace: &Path) {
    let (source, inputs) = match git_source(workspace) {
        Some(found) => found,
        None => {
            let file = workspace.join("COMMIT");
            match std::fs::read_to_string(&file) {
                Ok(text) => (version::Source::recorded(&text), vec![file]),
                Err(_) => (version::Source::Unknown, Vec::new()),
            }
        }
    };
    for path in version::SOURCES
        .iter()
        .map(|path| workspace.join(path))
        .chain(inputs)
        .filter(|path| path.exists())
    {
        println!("cargo::rerun-if-changed={}", path.display());
    }
    println!("cargo::rerun-if-env-changed=IGNITE_RELEASE_COMMIT");
    println!(
        "cargo::rustc-env=IGNITE_VERSION={}",
        version::describe(&env("CARGO_PKG_VERSION"), &source)
    );
}

/// The checkout's source, when `workspace` is a checkout's top level
/// (`None` without git or inside another repository), and the git files
/// that move when its `HEAD` does, all of them per worktree or per ref so
/// that another worktree's fetch, branch or gc reruns nothing here:
/// `HEAD` (a checkout of another branch), `logs/HEAD` (every commit, reset
/// and amend appends to it, whether the branch's ref is loose or packed),
/// the branch's loose ref (for a repository without a reflog), the
/// worktree's `reftable/tables.list` (the reftable backend, which keeps no
/// `logs/`) and the release tag's loose ref. A path that does not exist is
/// not watched, since cargo reruns a script whose watched path is missing at
/// every build.
fn git_source(workspace: &Path) -> Option<(version::Source, Vec<PathBuf>)> {
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .arg("--no-optional-locks")
            .arg("-C")
            .arg(workspace)
            .args(args)
            .output()
            .ok()?;
        output
            .status
            .success()
            .then(|| String::from_utf8(output.stdout).ok())
            .flatten()
            .map(|text| text.trim().to_string())
    };
    let top = PathBuf::from(git(&["rev-parse", "--show-toplevel"])?);
    if top.canonicalize().ok()? != workspace.canonicalize().ok()? {
        return None;
    }
    let commit = git(&["rev-parse", "HEAD"])?;
    let mut status = vec!["status", "--porcelain", "--untracked-files=no", "--"];
    status.extend(version::SOURCES);
    let dirty = !git(&status)?.is_empty();
    let tags = git(&["tag", "--points-at", "HEAD"])?
        .lines()
        .map(str::to_string)
        .collect();
    let dir = PathBuf::from(git(&["rev-parse", "--absolute-git-dir"])?);
    let common = PathBuf::from(git(&[
        "rev-parse",
        "--path-format=absolute",
        "--git-common-dir",
    ])?);
    let mut inputs = vec![
        dir.join("HEAD"),
        dir.join("logs/HEAD"),
        dir.join("reftable/tables.list"),
        common.join(format!("refs/tags/v{}", env("CARGO_PKG_VERSION"))),
    ];
    if let Some(branch) = git(&["symbolic-ref", "-q", "HEAD"]) {
        inputs.push(common.join(branch));
    }
    Some((
        version::Source::Git {
            commit,
            dirty,
            tags,
        },
        inputs,
    ))
}
