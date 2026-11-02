//! `ignite build --target llvm` (`docs/llvm.md` section 1): IR to
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

/// Flags of every `clang -c` after the optimization level and before
/// `--target=<triple>`. `-Werror=override-module` turns an IR triple that
/// differs from clang's target into an error: the IR is pinned to the
/// target clang compiles for.
pub const COMPILE_FLAGS: &[&str] = &[
    "-g",
    "-ffunction-sections",
    "-fdata-sections",
    "-Werror=override-module",
];

/// Optimization level of `clang -c` without `--release`
/// (`docs/llvm.md` section 1, measured there).
pub const DEV_OPTIMIZATION: &str = "-O0";

/// Optimization level of `clang -c` with `--release`.
pub const RELEASE_OPTIMIZATION: &str = "-O2";

/// The file in the IR directory that caches the target probe.
const TARGET_STAMP: &str = "target.stamp";

/// `path` itself when it has a directory part, else its first match on
/// `PATH`.
fn resolve_program(path: &Path) -> PathBuf {
    if path.components().count() > 1 {
        return path.to_path_buf();
    }
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .map(|dir| dir.join(path))
        .find(|candidate| candidate.is_file())
        .unwrap_or_else(|| path.to_path_buf())
}

/// Path, size and modification time of `path`; `missing` when it cannot be
/// read.
fn file_identity(path: &Path) -> String {
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    match std::fs::metadata(&canonical) {
        Ok(metadata) => {
            let modified = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|since| since.as_nanos())
                .unwrap_or(0);
            format!("{} {} {modified}", canonical.display(), metadata.len())
        }
        Err(_) => format!("{} missing", canonical.display()),
    }
}

/// The target triple and data layout every IR module of a build is pinned
/// to (`docs/llvm.md` section 1): those of `--triple`, else clang's
/// default target, read from the module clang emits for an empty C file
/// (`clang [--target=T] -S -emit-llvm -x c /dev/null -o -`). The answer is
/// cached in `ir_dir/target.stamp` under the clang command and the clang
/// binary's path, size and modification time, so a rebuild runs no clang.
pub(crate) fn target(
    options: &BuildOptions,
    ir_dir: &Path,
) -> Result<ignite_llvm::TargetSpec, Error> {
    let clang_path = options
        .clang
        .clone()
        .unwrap_or_else(|| PathBuf::from("clang"));
    let mut args: Vec<OsString> = Vec::new();
    if let Some(triple) = &options.triple {
        args.push(format!("--target={triple}").into());
    }
    args.extend(["-S", "-emit-llvm", "-x", "c", "/dev/null", "-o", "-"].map(OsString::from));
    let key = stamp(
        &clang_path,
        &args,
        &format!("clang {}\n", file_identity(&resolve_program(&clang_path))),
    );
    let stamp_path = ir_dir.join(TARGET_STAMP);
    if let Ok(cached) = std::fs::read_to_string(&stamp_path)
        && let Some(rest) = cached.strip_prefix(&key)
        && let Some(spec) = parse_target(rest)
    {
        return Ok(spec);
    }
    std::fs::create_dir_all(ir_dir).map_err(io_error(ir_dir))?;
    let output = Command::new(&clang_path)
        .args(&args)
        .stderr(std::process::Stdio::inherit())
        .output()
        .map_err(|error| Error::Io {
            path: clang_path.clone(),
            error,
        })?;
    if !output.status.success() {
        return Err(Error::Clang {
            status: output.status.code().unwrap_or(-1),
        });
    }
    let probe = String::from_utf8_lossy(&output.stdout);
    let Some(spec) = parse_target(&probe) else {
        return Err(Error::Io {
            path: clang_path,
            error: std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "the target probe printed no `target triple` and `target datalayout`",
            ),
        });
    };
    let text = format!(
        "{key}target triple = \"{}\"\ntarget datalayout = \"{}\"\n",
        spec.triple, spec.datalayout
    );
    std::fs::write(&stamp_path, text).map_err(io_error(&stamp_path))?;
    Ok(spec)
}

/// The `target triple` and `target datalayout` lines of an IR text.
fn parse_target(ir: &str) -> Option<ignite_llvm::TargetSpec> {
    let quoted = |prefix: &str| {
        ir.lines().find_map(|line| {
            let value = line.strip_prefix(prefix)?.trim();
            Some(value.strip_prefix('"')?.strip_suffix('"')?.to_string())
        })
    };
    Some(ignite_llvm::TargetSpec {
        triple: quoted("target triple =")?,
        datalayout: quoted("target datalayout =")?,
    })
}

/// The ELF class, byte order and machine an architecture's objects have,
/// by the first component of a triple; `None` for one this check does not
/// know.
fn elf_machine(arch: &str) -> Option<(u8, u8, u16)> {
    const LE: u8 = 1;
    const BE: u8 = 2;
    Some(match arch {
        "x86_64" | "amd64" => (2, LE, 62),
        "i386" | "i486" | "i586" | "i686" => (1, LE, 3),
        "aarch64" | "arm64" => (2, LE, 183),
        "aarch64_be" => (2, BE, 183),
        "riscv64" => (2, LE, 243),
        "riscv32" => (1, LE, 243),
        "powerpc64le" => (2, LE, 21),
        "powerpc64" => (2, BE, 21),
        "s390x" => (2, BE, 22),
        "loongarch64" => (2, LE, 258),
        _ if arch.starts_with("armv") || arch == "arm" || arch.starts_with("thumb") => (1, LE, 40),
        _ => return None,
    })
}

/// Class, byte order and machine of the first ELF member of an `ar`
/// archive.
fn archive_machine(archive: &[u8]) -> Option<(u8, u8, u16)> {
    let mut at = archive.strip_prefix(b"!<arch>\n").map(|_| 8)?;
    while at + 60 <= archive.len() {
        let header = &archive[at..at + 60];
        let size: usize = std::str::from_utf8(&header[48..58])
            .ok()?
            .trim()
            .parse()
            .ok()?;
        let data = archive.get(at + 60..at + 60 + size)?;
        if data.len() >= 20 && data.starts_with(b"\x7fELF") {
            let (class, order) = (data[4], data[5]);
            let bytes = [data[18], data[19]];
            let machine = if order == 2 {
                u16::from_be_bytes(bytes)
            } else {
                u16::from_le_bytes(bytes)
            };
            return Some((class, order, machine));
        }
        at += 60 + size + size % 2;
    }
    None
}

/// Fail unless the objects of the runtime archive are for `triple`'s
/// architecture (`docs/llvm.md` section 1). An architecture or an archive
/// this check does not know passes.
fn check_runtime_target(runtime: &Path, triple: &str) -> Result<(), Error> {
    let arch = triple.split('-').next().unwrap_or(triple);
    let Some(expected) = elf_machine(arch) else {
        return Ok(());
    };
    let archive = std::fs::read(runtime).map_err(io_error(runtime))?;
    let Some(found) = archive_machine(&archive) else {
        return Ok(());
    };
    if found == expected {
        return Ok(());
    }
    let (class, order, machine) = found;
    Err(Error::Io {
        path: runtime.to_path_buf(),
        error: std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "the runtime archive is not built for the target {triple}: its objects are ELF{} {} machine {machine}, the target needs machine {} (build ignite-rt for {triple} and pass it with --runtime)",
                if class == 2 { 64 } else { 32 },
                if order == 2 {
                    "big-endian"
                } else {
                    "little-endian"
                },
                expected.2
            ),
        ),
    })
}

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
    let (package, generated) = crate::emit_llvm_with(file, options.std.as_deref(), |package| {
        target(options, &out_dir(package, options).join("llvm")).map(Some)
    })?;
    link(&package, &generated, options)
}

/// The output directory of a build: `--out`, else `target/ignite/<package>`.
fn out_dir(package: &str, options: &BuildOptions) -> PathBuf {
    options
        .out
        .clone()
        .unwrap_or_else(|| Path::new("target").join("ignite").join(package))
}

/// Write the IR of `generated`, compile it for the target of `options`
/// ([`target`]) and link each executable. The runtime archive is checked
/// against the target before clang compiles anything.
pub(crate) fn link(
    package: &str,
    generated: &ignite_llvm::Generated,
    options: &BuildOptions,
) -> Result<BuildOutput, Error> {
    let out = out_dir(package, options);
    let ir_dir = out.join("llvm");
    let spec = target(options, &ir_dir)?;
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
    check_runtime_target(&runtime, &spec.triple)?;
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
    let optimization = if options.release {
        RELEASE_OPTIMIZATION
    } else {
        DEV_OPTIMIZATION
    };
    let mut object_stamps = std::collections::BTreeMap::new();
    for (name, ir) in &units {
        let ll = ir_dir.join(format!("{name}.ll"));
        write_if_changed(&ll, ir)?;
        let object = ir_dir.join(format!("{name}.o"));
        let mut args: Vec<OsString> = vec!["-c".into(), optimization.into()];
        args.extend(COMPILE_FLAGS.iter().map(Into::into));
        args.push(format!("--target={}", spec.triple).into());
        args.extend([ll.into_os_string(), "-o".into(), object.clone().into()]);
        let expected = stamp(&clang_path, &args, &format!("ir {:016x}\n", ir_hash(ir)));
        let stamp_path = ir_dir.join(format!("{name}.o.stamp"));
        run_stamped(&clang_path, &args, &object, &stamp_path, &expected)?;
        object_stamps.insert(name.clone(), expected);
    }
    let runtime_identity = runtime_identity(&runtime)?;
    let mut executables = Vec::new();
    for main in &generated.mains {
        let executable = bin_dir.join(&main.executable);
        let mut args: Vec<OsString> = vec![
            "-o".into(),
            executable.clone().into(),
            format!("--target={}", spec.triple).into(),
        ];
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
