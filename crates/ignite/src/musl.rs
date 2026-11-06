//! `--static` (`docs/slice.md` 23): executables linked against musl, with
//! no program interpreter and no shared library.
//!
//! The triple is `--triple`, which must name a musl target, else
//! `<arch>-unknown-linux-musl` for the driver's architecture (23.2). The
//! LLVM target links the musl start files, the runtime archive built for
//! that triple, `libunwind.a` and `libc.a` itself, with no system library
//! (23.3); the Rust target hands the triple to cargo. A musl target the
//! build cannot get is exit 2 naming it, never a dynamic build (23.4).

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{BuildOptions, Error};

/// The start files linked before the program's objects, in order.
const START_FILES: &[&str] = &["crt1.o", "crti.o", "crtbegin.o"];

/// The libraries and end files linked after the runtime archive, in order.
const END_FILES: &[&str] = &["libunwind.a", "libc.a", "crtend.o", "crtn.o"];

/// The musl triple of a build, `None` without `--static`. Exit 2 for a
/// `--triple` that names no musl target.
pub(crate) fn triple(options: &BuildOptions) -> Result<Option<String>, Error> {
    if !options.static_link {
        return Ok(None);
    }
    match &options.triple {
        None => Ok(Some(format!(
            "{}-unknown-linux-musl",
            std::env::consts::ARCH
        ))),
        Some(triple) if triple.split('-').any(|part| part.starts_with("musl")) => {
            Ok(Some(triple.clone()))
        }
        Some(triple) => Err(Error::Toolchain {
            message: format!(
                "--static links against musl, and --triple {triple} is no musl target; give one such as {}-unknown-linux-musl or drop --static",
                std::env::consts::ARCH
            ),
        }),
    }
}

/// The triple clang compiles for: the musl triple with `--static`, else
/// `--triple`.
pub(crate) fn clang_triple(options: &BuildOptions) -> Result<Option<String>, Error> {
    Ok(triple(options)?.or_else(|| options.triple.clone()))
}

/// What a static LLVM link needs: the directory holding [`START_FILES`]
/// and [`END_FILES`], and the runtime archive built for the triple.
pub(crate) struct Kit {
    dir: PathBuf,
    pub(crate) runtime: PathBuf,
}

impl Kit {
    /// The link arguments before the objects.
    pub(crate) fn before(&self) -> Vec<OsString> {
        let mut args: Vec<OsString> = vec!["-static".into(), "-nostdlib".into()];
        args.extend(START_FILES.iter().map(|name| self.dir.join(name).into()));
        args
    }

    /// The link arguments after the runtime archive and the link flags.
    pub(crate) fn after(&self) -> Vec<OsString> {
        END_FILES
            .iter()
            .map(|name| self.dir.join(name).into())
            .collect()
    }
}

/// The kit of a static LLVM build for `triple`: the one this driver
/// carries for it (written to the runtime cache), else the
/// `self-contained` directory of the installed Rust target, whose runtime
/// archive `--runtime` must then name. `--runtime` replaces the carried
/// archive. Exit 2 when neither has the triple.
pub(crate) fn kit(options: &BuildOptions, triple: &str) -> Result<Kit, Error> {
    if let Some(dir) = crate::embedded_runtime::musl_kit(triple)? {
        let runtime = options
            .runtime
            .clone()
            .unwrap_or_else(|| dir.join("libignite_rt.a"));
        return Ok(Kit { dir, runtime });
    }
    let dir = rust_libdir(triple)?.join("self-contained");
    if let Some(missing) = START_FILES
        .iter()
        .chain(END_FILES)
        .map(|name| dir.join(name))
        .find(|path| !path.is_file())
    {
        return Err(Error::Toolchain {
            message: format!(
                "--static needs {} of the Rust target {triple}, which is missing: reinstall the target with `rustup target add {triple}`",
                missing.display()
            ),
        });
    }
    let Some(runtime) = options.runtime.clone() else {
        let carried = crate::embedded_runtime::musl_triple().map_or_else(
            || "carries no static runtime: it was built without the Rust target".to_string(),
            |carried| format!("carries the static runtime of {carried} only"),
        );
        return Err(Error::Toolchain {
            message: format!(
                "this ignite {carried}; pass a runtime for {triple} with --runtime (built with `cargo build -p ignite-rt --features runner --target {triple}`), or rebuild ignite where `rustup target add {triple}` is installed"
            ),
        });
    };
    Ok(Kit { dir, runtime })
}

/// Exit 2 unless the Rust target `triple` is installed: the Rust target's
/// `cargo build --target` needs its standard library.
pub(crate) fn require_rust_target(triple: &str) -> Result<(), Error> {
    rust_libdir(triple).map(drop)
}

/// The library directory of the installed Rust target `triple`, from
/// `rustc --print target-libdir` (`$RUSTC`, else `rustc` on `PATH`); exit 2
/// naming `rustup target add` when it holds no standard library.
fn rust_libdir(triple: &str) -> Result<PathBuf, Error> {
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let libdir = Command::new(&rustc)
        .args(["--print", "target-libdir", "--target", triple])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| PathBuf::from(String::from_utf8_lossy(&output.stdout).trim()));
    match libdir {
        Some(libdir) if has_std(&libdir) => Ok(libdir),
        _ => Err(Error::Toolchain {
            message: format!(
                "--static links against musl and needs the Rust target {triple}, which is not installed: install it with `rustup target add {triple}`"
            ),
        }),
    }
}

fn has_std(libdir: &Path) -> bool {
    std::fs::read_dir(libdir).is_ok_and(|mut entries| {
        entries.any(|entry| {
            entry.is_ok_and(|entry| entry.file_name().to_string_lossy().starts_with("libstd-"))
        })
    })
}
