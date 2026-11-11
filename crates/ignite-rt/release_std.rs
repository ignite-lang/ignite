//! How a release runtime archive of the LLVM target is built (story
//! `binary-size-back`, row 3.9 of `docs/quality.md`): on a standard library
//! compiled from its source, the toolchain's `rust-src` component, without
//! its backtrace symbolizer, about 130 KB of every executable. An Ignite
//! program reports its failures itself; a panic inside the runtime still
//! prints its message and location, its backtrace unsymbolized. The flags
//! are unstable cargo flags, which a stable cargo takes with
//! `RUSTC_BOOTSTRAP=1`; `-Zbuild-std` needs `--target`.
//!
//! One recipe for every build of that archive: the driver's build script
//! in the release profile (`task dist`, `cargo install`, a source build),
//! and `ignite-perf runtime` for the dedicated host's benchmark and `task
//! bench-throughput`. Both include this file with `#[path]`. Without
//! `rust-src` the archive is built on the prebuilt standard library and the
//! build says so ([`MISSING`]).

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The cargo arguments: `std` and `panic_unwind` from source, with the
/// standard library's features but `backtrace`.
pub const ARGS: &[&str] = &[
    "-Zbuild-std=std,panic_unwind",
    "-Zbuild-std-features=panic-unwind",
];

/// The environment that lets a stable cargo take [`ARGS`].
pub const ENV: (&str, &str) = ("RUSTC_BOOTSTRAP", "1");

/// What a release runtime build without the source says.
pub const MISSING: &str = "the toolchain has no rust-src component (rustup component add rust-src): the release runtime is built on the prebuilt standard library, with its backtrace symbolizer, about 130 KB more per LLVM executable";

/// The sysroot of `rustc`, `rustc --print sysroot`.
pub fn sysroot(rustc: &OsStr) -> Option<PathBuf> {
    let output = Command::new(rustc)
        .args(["--print", "sysroot"])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8(output.stdout).ok())
        .flatten()
        .map(|text| PathBuf::from(text.trim()))
}

/// The source of the standard library under `sysroot`, when it is
/// installed: `lib/rustlib/src/rust/library`, with `std` in it.
pub fn library(sysroot: &Path) -> Option<PathBuf> {
    let library = sysroot.join("lib/rustlib/src/rust/library");
    library.join("std/Cargo.toml").is_file().then_some(library)
}

/// Add [`ARGS`] and [`ENV`] to `cargo`, a `cargo build` with `--target`.
pub fn apply(cargo: &mut Command) {
    cargo.args(ARGS).env(ENV.0, ENV.1);
}
