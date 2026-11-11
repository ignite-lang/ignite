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
use std::io::{Read, Seek, SeekFrom};
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

/// `clang -c` flag of a release build with LTO: each unit is compiled to
/// bitcode for ThinLTO, and lld optimises and generates code for it
/// together with the bitcode runtime at the link.
pub const LTO_COMPILE_FLAG: &str = "-flto=thin";

/// Link flags of a release build with LTO, before the inputs: ThinLTO at
/// `-O2` through lld, which reads LLVM bitcode without a linker plugin.
/// The ThinLTO cache (`--thinlto-cache-dir`, in the IR directory) lets the
/// second executable of a program and a relink reuse the backend work of
/// modules whose inputs did not change; it is pruned to 64 MiB.
pub const LTO_LINK_FLAGS: &[&str] = &[
    "-flto=thin",
    "-O2",
    "-fuse-ld=lld",
    "-Wl,--thinlto-cache-policy=cache_size_bytes=64m",
];

/// The file in the IR directory that caches the target probe.
const TARGET_STAMP: &str = "target.stamp";

/// The ThinLTO cache directory in the IR directory.
const LTO_CACHE: &str = "lto-cache";

/// The bitcode runtime a build links with LTO, before clang's LLVM
/// version is known: with `--release`, without `--runtime`, `--triple` and
/// `--static`, when the driver carries one (`build.rs`) and `ld.lld` lies
/// next to clang or on `PATH`. Every other build links the native archive
/// without LTO, as before.
fn lto_candidate(options: &BuildOptions) -> Option<&'static crate::embedded_runtime::LtoRuntime> {
    if !options.release
        || options.runtime.is_some()
        || options.triple.is_some()
        || options.static_link
    {
        return None;
    }
    let runtime = crate::embedded_runtime::lto_runtime()?;
    let clang = resolve_program(options.clang.as_deref().unwrap_or(Path::new("clang")));
    let beside = clang.parent().map(|dir| dir.join("ld.lld"));
    let lld = beside
        .filter(|path| path.is_file())
        .unwrap_or_else(|| resolve_program(Path::new("ld.lld")));
    lld.is_file().then_some(runtime)
}

/// The cached bitcode runtime of a build that links with LTO: a
/// [`lto_candidate`] whose rustc used the major LLVM version clang reports
/// (`major`, from the target probe), since lld reads bitcode only of its
/// own LLVM. `None` links the native archive without LTO.
fn lto_runtime(options: &BuildOptions, major: Option<u32>) -> Option<PathBuf> {
    let runtime = lto_candidate(options)?;
    if major != Some(runtime.llvm_major) {
        return None;
    }
    crate::embedded_runtime::materialize_lto()
}

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
/// to (`docs/llvm.md` section 1): those of `--triple` (with `--static` the musl
/// triple of `docs/slice.md` 23.2), else clang's
/// default target, read from the module clang emits for an empty C file
/// (`clang [--target=T] -S -emit-llvm -x c /dev/null -o -`). The answer is
/// cached in `ir_dir/target.stamp` under the clang command and the clang
/// binary's path, size and modification time, so a rebuild runs no clang.
/// A build that may link with LTO ([`lto_candidate`]) is pinned to the
/// triple of the bitcode runtime instead (`x86_64-unknown-linux-gnu` for
/// clang's `x86_64-pc-linux-gnu`: one target, two spellings), so that lld
/// links modules of one triple.
pub(crate) fn target(
    options: &BuildOptions,
    ir_dir: &Path,
) -> Result<ignite_llvm::TargetSpec, Error> {
    Ok(probe(options, ir_dir)?.0)
}

/// [`target`], and the major LLVM version of clang, read from the
/// `llvm.ident` of the same probe (`clang version X.Y.Z`); `None` when the
/// probe names none.
fn probe(
    options: &BuildOptions,
    ir_dir: &Path,
) -> Result<(ignite_llvm::TargetSpec, Option<u32>), Error> {
    let clang_path = options
        .clang
        .clone()
        .unwrap_or_else(|| PathBuf::from("clang"));
    let mut args: Vec<OsString> = Vec::new();
    if let Some(triple) = crate::musl::clang_triple(options)? {
        args.push(format!("--target={triple}").into());
    } else if let Some(runtime) = lto_candidate(options) {
        args.push(format!("--target={}", runtime.triple).into());
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
        && let Some(version) = rest
            .lines()
            .find_map(|line| line.strip_prefix(CLANG_VERSION))
    {
        return Ok((spec, llvm_major(version)));
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
    let version = probe
        .lines()
        .filter(|line| line.starts_with('!'))
        .find_map(|line| {
            let rest = line.split("clang version ").nth(1)?;
            Some(rest.split(['"', ' ']).next()?.to_string())
        })
        .unwrap_or_default();
    let text = format!(
        "{key}target triple = \"{}\"\ntarget datalayout = \"{}\"\n{CLANG_VERSION}{version}\n",
        spec.triple, spec.datalayout
    );
    std::fs::write(&stamp_path, text).map_err(io_error(&stamp_path))?;
    Ok((spec, llvm_major(&version)))
}

/// The line of the target stamp after which clang's version stands.
const CLANG_VERSION: &str = "clang version = ";

/// The leading number of a version text `X.Y.Z…`.
fn llvm_major(version: &str) -> Option<u32> {
    let digits: String = version
        .trim_start()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
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
fn archive_machine(archive: &mut (impl Read + Seek)) -> std::io::Result<Option<(u8, u8, u16)>> {
    let length = archive.seek(SeekFrom::End(0))?;
    if length < 8 {
        return Ok(None);
    }
    archive.seek(SeekFrom::Start(0))?;
    let mut magic = [0; 8];
    archive.read_exact(&mut magic)?;
    if &magic != b"!<arch>\n" {
        return Ok(None);
    }
    let mut at = 8_u64;
    while let Some(start) = at.checked_add(60).filter(|&start| start <= length) {
        archive.seek(SeekFrom::Start(at))?;
        let mut header = [0; 60];
        archive.read_exact(&mut header)?;
        let Some(size) = std::str::from_utf8(&header[48..58])
            .ok()
            .and_then(|size| size.trim().parse::<usize>().ok())
        else {
            return Ok(None);
        };
        // A prefix is classified only when the entire declared member is
        // present, as in the former whole-archive slice walk.
        let Some(end) = start.checked_add(size as u64).filter(|&end| end <= length) else {
            return Ok(None);
        };
        if size >= 20 {
            let mut data = [0; 20];
            archive.read_exact(&mut data)?;
            if data.starts_with(b"\x7fELF") {
                let (class, order) = (data[4], data[5]);
                let bytes = [data[18], data[19]];
                let machine = if order == 2 {
                    u16::from_be_bytes(bytes)
                } else {
                    u16::from_le_bytes(bytes)
                };
                return Ok(Some((class, order, machine)));
            }
        }
        let Some(next) = end.checked_add((size % 2) as u64) else {
            return Ok(None);
        };
        at = next;
    }
    Ok(None)
}

/// Fail unless the objects of the runtime archive are for `triple`'s
/// architecture (`docs/llvm.md` section 1). An architecture or an archive
/// this check does not know passes.
fn check_runtime_target(runtime: &Path, triple: &str) -> Result<(), Error> {
    let arch = triple.split('-').next().unwrap_or(triple);
    let Some(expected) = elf_machine(arch) else {
        return Ok(());
    };
    let mut archive = std::fs::File::open(runtime).map_err(io_error(runtime))?;
    let Some(found) = archive_machine(&mut archive).map_err(io_error(runtime))? else {
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

/// Link flags of a dev build that finds lld ([`dev_lld`]), before the
/// inputs. A dev link copies and relocates the debug sections of every
/// archive member it loads; lld maps the archive and works on all cores
/// where GNU ld reads it section by section on one (`docs/llvm.md`
/// section 1, row 6.2 of `docs/quality.md`).
pub const DEV_LLD_FLAGS: &[&str] = &["-fuse-ld=lld"];

/// Whether a build links with lld without LTO: a dev build without
/// `--triple` and `--static` whose clang (`clang`, resolved on `PATH` and
/// through its symbolic links, as clang finds its own tools) has `ld.lld`
/// beside it. Every other build links with clang's default linker.
fn dev_lld(options: &BuildOptions, clang: &Path) -> bool {
    if options.release || options.triple.is_some() || options.static_link {
        return false;
    }
    let resolved = resolve_program(clang);
    let canonical = std::fs::canonicalize(&resolved).unwrap_or(resolved);
    canonical
        .parent()
        .is_some_and(|dir| dir.join("ld.lld").is_file())
}

/// The arguments of every link of a build before its objects: the musl
/// kit's start files with `--static`, ThinLTO with the bitcode runtime
/// ([`LTO_LINK_FLAGS`]), else lld on a dev build that finds it
/// ([`DEV_LLD_FLAGS`]).
fn link_flags_before(
    options: &BuildOptions,
    kit: Option<&crate::musl::Kit>,
    lto: bool,
    ir_dir: &Path,
    clang: &Path,
) -> Vec<OsString> {
    let mut args: Vec<OsString> = Vec::new();
    if let Some(kit) = kit {
        args.extend(kit.before());
    }
    if lto {
        args.extend(LTO_LINK_FLAGS.iter().map(Into::into));
        let cache = ir_dir.join(LTO_CACHE);
        args.push(format!("-Wl,--thinlto-cache-dir={}", cache.display()).into());
    } else if kit.is_none() && dev_lld(options, clang) {
        args.extend(DEV_LLD_FLAGS.iter().map(Into::into));
    }
    args
}

/// The arguments of every link of a build after its objects and the
/// runtime archive: [`LINK_FLAGS`], [`RELEASE_LINK_FLAGS`] with
/// `--release`, then the musl kit's end files or [`RUNTIME_LIBS`].
fn link_flags_after(options: &BuildOptions, kit: Option<&crate::musl::Kit>) -> Vec<OsString> {
    let mut args: Vec<OsString> = LINK_FLAGS.iter().map(Into::into).collect();
    if options.release {
        args.extend(RELEASE_LINK_FLAGS.iter().map(Into::into));
    }
    match kit {
        Some(kit) => args.extend(kit.after()),
        None => args.extend(RUNTIME_LIBS.iter().map(Into::into)),
    }
    args
}

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

/// Exit 3 for a runtime archive that is not a file, naming both ways out:
/// build the archive (with the test runner, which `ignite test` also
/// needs) and pass it, or build with the Rust target, which needs none.
pub(crate) fn missing_runtime(runtime: PathBuf) -> Error {
    Error::Io {
        path: runtime,
        error: std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "runtime archive not found: the LLVM target, the default, links it; build it in the ignite source tree with `cargo build -p ignite-rt --features runner` and pass it with --runtime, or build with --target rust",
        ),
    }
}

/// Write the IR of `generated`, compile it for the target of `options`
/// ([`target`]) and link each executable. The runtime archive is checked
/// against the target before clang compiles anything. With `--static` the
/// musl kit is found first (`docs/slice.md` 23.3): the executables link
/// its start files, the runtime archive, `libunwind.a` and `libc.a` with
/// `-static -nostdlib` instead of [`RUNTIME_LIBS`].
pub(crate) fn link(
    package: &str,
    generated: &ignite_llvm::Generated,
    options: &BuildOptions,
) -> Result<BuildOutput, Error> {
    let kit = match crate::musl::triple(options)? {
        Some(triple) => Some(crate::musl::kit(options, &triple)?),
        None => None,
    };
    let out = out_dir(package, options);
    let ir_dir = out.join("llvm");
    let (spec, clang_major) = probe(options, &ir_dir)?;
    let lto = lto_runtime(options, clang_major);
    let runtime = match (&kit, &lto) {
        (Some(kit), _) => kit.runtime.clone(),
        (None, Some(lto)) => lto.clone(),
        (None, None) => crate::runtime_or_default(options.runtime.as_ref())?,
    };
    if !runtime.is_file() {
        return Err(missing_runtime(runtime));
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
        if lto.is_some() {
            args.push(LTO_COMPILE_FLAG.into());
        }
        args.extend(COMPILE_FLAGS.iter().map(Into::into));
        args.push(format!("--target={}", spec.triple).into());
        args.extend([ll.into_os_string(), "-o".into(), object.clone().into()]);
        let expected = stamp(&clang_path, &args, &format!("ir {:016x}\n", ir_hash(ir)));
        let stamp_path = ir_dir.join(format!("{name}.o.stamp"));
        run_stamped(&clang_path, &args, &object, &stamp_path, &expected)?;
        object_stamps.insert(name.clone(), expected);
    }
    let runtime_identity = runtime_identity(&runtime)?;
    let before = link_flags_before(options, kit.as_ref(), lto.is_some(), &ir_dir, &clang_path);
    let after = link_flags_after(options, kit.as_ref());
    let mut executables = Vec::new();
    for main in &generated.mains {
        let executable = bin_dir.join(&main.executable);
        let mut args: Vec<OsString> = vec![
            "-o".into(),
            executable.clone().into(),
            format!("--target={}", spec.triple).into(),
        ];
        args.extend(before.iter().cloned());
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
        args.extend(after.iter().cloned());
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

#[cfg(test)]
mod archive_tests {
    use super::*;
    use std::io::Cursor;

    fn member(archive: &mut Vec<u8>, data: &[u8]) {
        let header = format!(
            "{:<16}{:<12}{:<6}{:<6}{:<8}{:<10}`\n",
            "member/",
            0,
            0,
            0,
            "644",
            data.len()
        );
        assert_eq!(header.len(), 60);
        archive.extend_from_slice(header.as_bytes());
        archive.extend_from_slice(data);
        if !data.len().is_multiple_of(2) {
            archive.push(b'\n');
        }
    }

    #[test]
    fn first_elf_after_odd_and_large_members_preserves_byte_order() {
        for (order, encoded) in [(1, 183_u16.to_le_bytes()), (2, 183_u16.to_be_bytes())] {
            let mut archive = b"!<arch>\n".to_vec();
            member(&mut archive, b"odd");
            member(&mut archive, &[0; 32_768]);
            let mut elf = [0; 32];
            elf[..4].copy_from_slice(b"\x7fELF");
            elf[4] = 2;
            elf[5] = order;
            elf[18..20].copy_from_slice(&encoded);
            member(&mut archive, &elf);
            elf[18..20].copy_from_slice(&62_u16.to_le_bytes());
            member(&mut archive, &elf);
            assert_eq!(
                archive_machine(&mut Cursor::new(archive)).unwrap(),
                Some((2, order, 183))
            );
        }
    }

    #[test]
    fn truncated_member_with_complete_elf_prefix_remains_unknown() {
        let mut archive = b"!<arch>\n".to_vec();
        let mut elf = [0; 100];
        elf[..4].copy_from_slice(b"\x7fELF");
        elf[4] = 2;
        elf[5] = 1;
        elf[18..20].copy_from_slice(&183_u16.to_le_bytes());
        member(&mut archive, &elf);
        for length in [0, 7, 8, 67, 88, archive.len() - 1] {
            assert_eq!(
                archive_machine(&mut Cursor::new(&archive[..length])).unwrap(),
                None
            );
        }
        archive[8 + 48..8 + 58].fill(b'?');
        assert_eq!(archive_machine(&mut Cursor::new(archive)).unwrap(), None);
    }
}
