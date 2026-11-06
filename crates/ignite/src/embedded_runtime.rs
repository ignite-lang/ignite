//! The runtime archive the driver carries: `libignite_rt.a` with the test
//! runner, built by `build.rs` for the driver's own target and profile.
//! [`crate::default_runtime`] falls back to it when no archive lies next to
//! the driver; it is written once per content hash to
//! `$XDG_CACHE_HOME/ignite/runtime/<hash>/libignite_rt.a` (`~/.cache` when
//! `XDG_CACHE_HOME` is unset or not absolute), through a temporary file and
//! a rename, so that concurrent drivers never link a partial archive.
//!
//! A driver built where the Rust musl target is installed also carries the
//! musl kit of `ignite build --static` (`docs/slice.md` 23.3): the runtime
//! archive built for that target, the musl start files, `libc.a` and
//! `libunwind.a`. [`musl_kit`] writes it the same way, one file at a time,
//! to `<cache>/<kit hash>/`.

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::Error;

static ARCHIVE: &[u8] = include_bytes!(env!("IGNITE_RT_ARCHIVE"));

/// The content hash of [`ARCHIVE`], the name of its cache directory.
const HASH: &str = env!("IGNITE_RT_HASH");

/// The musl kit `build.rs` found: the triple, the hash of every file, and
/// each file's name and bytes, `libignite_rt.a` first.
pub(crate) struct Kit {
    pub(crate) triple: &'static str,
    hash: &'static str,
    files: &'static [(&'static str, &'static [u8])],
}

include!(concat!(env!("OUT_DIR"), "/musl_kit.rs"));

/// `$XDG_CACHE_HOME/ignite/runtime`, else `$HOME/.cache/ignite/runtime`.
pub(crate) fn cache_root() -> Option<PathBuf> {
    let absolute = |name: &str| {
        std::env::var_os(name)
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
    };
    absolute("XDG_CACHE_HOME")
        .or_else(|| absolute("HOME").map(|home| home.join(".cache")))
        .map(|cache| cache.join("ignite/runtime"))
}

/// The cached copy of the carried archive, written first if it is missing
/// or of another size. `looked` is the archive looked for next to the
/// driver, named in the error.
pub(crate) fn materialize(looked: &Path) -> Result<PathBuf, Error> {
    let Some(root) = cache_root() else {
        return Err(unwritable(
            looked,
            PathBuf::from("$HOME/.cache/ignite/runtime"),
            std::io::Error::other("neither XDG_CACHE_HOME nor HOME is an absolute path"),
        ));
    };
    let dir = root.join(HASH);
    let path = dir.join("libignite_rt.a");
    if std::fs::metadata(&path).is_ok_and(|metadata| metadata.len() == ARCHIVE.len() as u64) {
        return Ok(path);
    }
    write(&dir, &path, ARCHIVE).map_err(|error| unwritable(looked, path.clone(), error))?;
    Ok(path)
}

/// The triple of the carried musl kit, if this driver carries one.
pub(crate) fn musl_triple() -> Option<&'static str> {
    MUSL_KIT.as_ref().map(|kit| kit.triple)
}

/// The directory of the carried musl kit for `triple`, every file written
/// first if it is missing or of another size; `None` when this driver
/// carries no kit for `triple`.
pub(crate) fn musl_kit(triple: &str) -> Result<Option<PathBuf>, Error> {
    let Some(kit) = MUSL_KIT.as_ref().filter(|kit| kit.triple == triple) else {
        return Ok(None);
    };
    let Some(root) = cache_root() else {
        return Err(Error::Io {
            path: PathBuf::from("$HOME/.cache/ignite/runtime"),
            error: std::io::Error::other(
                "the musl kit of --static could not be written: neither XDG_CACHE_HOME nor HOME is an absolute path",
            ),
        });
    };
    let dir = root.join(format!("{}-{}", kit.triple, kit.hash));
    for (name, bytes) in kit.files {
        let path = dir.join(name);
        if std::fs::metadata(&path).is_ok_and(|metadata| metadata.len() == bytes.len() as u64) {
            continue;
        }
        write(&dir, &path, bytes).map_err(|error| Error::Io {
            path: path.clone(),
            error: std::io::Error::other(format!(
                "the musl kit of --static could not be written: {error}"
            )),
        })?;
    }
    Ok(Some(dir))
}

fn write(dir: &Path, path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.subsec_nanos());
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let temporary = dir.join(format!(".{name}.{}.{nanos}.tmp", std::process::id()));
    let result = (|| {
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

fn unwritable(looked: &Path, path: PathBuf, error: std::io::Error) -> Error {
    Error::Io {
        path,
        error: std::io::Error::other(format!(
            "no runtime archive at {} and the runtime this ignite carries could not be written: {error}; pass an archive with --runtime (built with `cargo build -p ignite-rt --features runner`), or build with --target rust",
            looked.display()
        )),
    }
}
