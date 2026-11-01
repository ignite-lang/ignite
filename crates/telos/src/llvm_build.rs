//! `telos build --target llvm` (`docs/llvm.md` section 1): IR to
//! `<out>/llvm`, objects next to it, executables in `<out>/bin`.
//!
//! Each step leaves a stamp in `<out>/llvm` after it succeeds: `<unit>.o.stamp`
//! holds the clang command and a hash of the unit's IR text,
//! `<exe>.link.stamp` the link command, the stamps of its objects and the
//! runtime archive's path, size and modification time. A step whose stamp
//! matches and whose output exists is skipped. The stamp is removed before
//! the step runs, so a failed or interrupted step runs again next time.
//! The runtime archive is identified by its metadata rather than a hash
//! because hashing it costs more than a whole unchanged rebuild.

use std::ffi::OsString;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{BuildOptions, BuildOutput, Error, RUNTIME_LIBS, io_error};

/// Flags of every `clang -c` after the optimization level.
pub const COMPILE_FLAGS: &[&str] = &[
    "-ffunction-sections",
    "-fdata-sections",
    "-Wno-override-module",
];

/// Link flags before the runtime libraries; `--release` adds
/// [`RELEASE_LINK_FLAGS`].
pub const LINK_FLAGS: &[&str] = &["-Wl,--gc-sections"];

/// Debug information is dropped from release executables, as cargo's
/// release profile does (`strip = "debuginfo"`).
pub const RELEASE_LINK_FLAGS: &[&str] = &["-Wl,--strip-debug"];

/// Run one clang command; its output is inherited.
fn clang(clang: &Path, args: &[OsString]) -> Result<(), Error> {
    let status = Command::new(clang)
        .args(args)
        .status()
        .map_err(|error| Error::Io {
            path: clang.to_path_buf(),
            error,
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::Clang {
            status: status.code().unwrap_or(-1),
        })
    }
}

/// The stamp of a command: the program and its arguments, one per line,
/// then `extra`.
fn stamp(program: &Path, args: &[OsString], extra: &str) -> String {
    let mut text = program.to_string_lossy().into_owned();
    for arg in args {
        text.push('\n');
        text.push_str(&arg.to_string_lossy());
    }
    text.push('\n');
    text.push_str(extra);
    text
}

/// Whether `output` exists and `stamp_path` holds exactly `expected`.
fn up_to_date(output: &Path, stamp_path: &Path, expected: &str) -> bool {
    output.is_file() && std::fs::read_to_string(stamp_path).is_ok_and(|current| current == expected)
}

/// Run `clang args` unless `stamp_path` already records `expected` for an
/// existing `output`. Returns whether clang ran.
fn run_stamped(
    clang_path: &Path,
    args: &[OsString],
    output: &Path,
    stamp_path: &Path,
    expected: &str,
) -> Result<bool, Error> {
    if up_to_date(output, stamp_path, expected) {
        return Ok(false);
    }
    match std::fs::remove_file(stamp_path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(io_error(stamp_path)(error)),
    }
    clang(clang_path, args)?;
    std::fs::write(stamp_path, expected).map_err(io_error(stamp_path))?;
    Ok(true)
}

fn ir_hash(ir: &str) -> u64 {
    let mut hasher = std::hash::DefaultHasher::new();
    ir.hash(&mut hasher);
    hasher.finish()
}

/// Path, size and modification time of the runtime archive.
fn runtime_identity(runtime: &Path) -> Result<String, Error> {
    let metadata = std::fs::metadata(runtime).map_err(io_error(runtime))?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|since| since.as_nanos())
        .unwrap_or(0);
    let path = std::fs::canonicalize(runtime).unwrap_or_else(|_| runtime.to_path_buf());
    Ok(format!(
        "runtime {} {} {modified}\n",
        path.display(),
        metadata.len()
    ))
}

/// Write `contents` to `path` unless the file already holds it.
fn write_if_changed(path: &Path, contents: &str) -> Result<(), Error> {
    if std::fs::read(path).is_ok_and(|current| current == contents.as_bytes()) {
        return Ok(());
    }
    std::fs::write(path, contents).map_err(io_error(path))
}

pub(crate) fn build(file: &Path, options: &BuildOptions) -> Result<BuildOutput, Error> {
    let (package, generated) = crate::emit_llvm(file, options.std.as_deref())?;
    link(&package, &generated, options)
}

/// Write the IR of `generated`, compile it and link each executable.
pub(crate) fn link(
    package: &str,
    generated: &telos_llvm::Generated,
    options: &BuildOptions,
) -> Result<BuildOutput, Error> {
    let out = options
        .out
        .clone()
        .unwrap_or_else(|| Path::new("target").join("telos").join(package));
    let ir_dir = out.join("llvm");
    let bin_dir = out.join("bin");
    for dir in [&ir_dir, &bin_dir] {
        std::fs::create_dir_all(dir).map_err(io_error(dir))?;
    }
    let mut units = vec![
        ("std".to_string(), &generated.std),
        ("program".to_string(), &generated.program),
    ];
    for main in &generated.mains {
        units.push((format!("main_{}", main.executable), &main.ir));
    }
    let clang_path = options
        .clang
        .clone()
        .unwrap_or_else(|| PathBuf::from("clang"));
    let optimization = if options.release { "-O2" } else { "-O1" };
    let mut object_stamps = std::collections::BTreeMap::new();
    for (name, ir) in &units {
        let ll = ir_dir.join(format!("{name}.ll"));
        write_if_changed(&ll, ir)?;
        let object = ir_dir.join(format!("{name}.o"));
        let mut args: Vec<OsString> = vec!["-c".into(), optimization.into()];
        args.extend(COMPILE_FLAGS.iter().map(Into::into));
        args.extend([ll.into_os_string(), "-o".into(), object.clone().into()]);
        let expected = stamp(&clang_path, &args, &format!("ir {:016x}\n", ir_hash(ir)));
        let stamp_path = ir_dir.join(format!("{name}.o.stamp"));
        run_stamped(&clang_path, &args, &object, &stamp_path, &expected)?;
        object_stamps.insert(name.clone(), expected);
    }
    let runtime = options
        .runtime
        .clone()
        .unwrap_or_else(crate::default_runtime_path);
    if !runtime.is_file() {
        return Err(Error::Io {
            path: runtime,
            error: std::io::Error::new(std::io::ErrorKind::NotFound, "runtime archive not found"),
        });
    }
    let runtime_identity = runtime_identity(&runtime)?;
    let mut executables = Vec::new();
    for main in &generated.mains {
        let executable = bin_dir.join(&main.executable);
        let mut args: Vec<OsString> = vec!["-o".into(), executable.clone().into()];
        let mut inputs = String::new();
        for unit in [
            format!("main_{}", main.executable),
            "program".into(),
            "std".into(),
        ] {
            args.push(ir_dir.join(format!("{unit}.o")).into_os_string());
            inputs.push_str(&object_stamps[&unit]);
        }
        args.push(runtime.clone().into_os_string());
        args.extend(LINK_FLAGS.iter().map(Into::into));
        if options.release {
            args.extend(RELEASE_LINK_FLAGS.iter().map(Into::into));
        }
        args.extend(RUNTIME_LIBS.iter().map(Into::into));
        inputs.push_str(&runtime_identity);
        let expected = stamp(&clang_path, &args, &inputs);
        let stamp_path = ir_dir.join(format!("{}.link.stamp", main.executable));
        run_stamped(&clang_path, &args, &executable, &stamp_path, &expected)?;
        executables.push(executable);
    }
    Ok(BuildOutput {
        project_dir: out,
        executables,
    })
}
