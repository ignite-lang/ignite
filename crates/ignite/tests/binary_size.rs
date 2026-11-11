//! Story `binary-size-back` (rows 3.2, 3.8 and 3.9 of `docs/quality.md`):
//! what the links of both targets write into an executable, read from the
//! executable itself.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crate::common;

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn run(command: &mut Command) -> Output {
    let output = command.output().expect("start the command");
    assert!(
        output.status.success(),
        "{command:?}\nstdout: {}\nstderr: {}",
        text(&output.stdout),
        text(&output.stderr)
    );
    output
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/backtrace.ignite")
}

/// `ignite build <fixture>` on `target` into `out`, dev or release; the
/// path of its executable `overflow`.
fn build(target: &str, release: bool, out: &Path, cargo_target: &Path) -> PathBuf {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ignite"));
    command
        .arg("build")
        .arg(fixture())
        .args(["--target", target])
        .arg("--out")
        .arg(out);
    if target == "llvm" {
        command.arg("--runtime").arg(common::runtime_archive());
    } else {
        command.arg("--target-dir").arg(cargo_target);
    }
    if release {
        command.arg("--release");
    }
    run(&mut command);
    match (target, release) {
        ("llvm", _) => out.join("bin/overflow"),
        (_, true) => cargo_target.join("release/overflow"),
        (_, false) => cargo_target.join("debug/overflow"),
    }
}

/// `readelf <flag> <file>`.
fn readelf(flag: &str, file: &Path) -> String {
    text(&run(Command::new("readelf").arg(flag).arg(file)).stdout)
}

/// Row 3.9 and 3.2: an executable of either target, dev and release,
/// keeps its relative relocations packed (`DT_RELR`, the linker's
/// `-z pack-relative-relocs`) and names the loader version that reads
/// them (`GLIBC_ABI_DT_RELR`, glibc 2.36), so a loader without it refuses
/// the executable instead of running it unrelocated.
#[test]
fn executables_pack_their_relative_relocations_on_both_targets() {
    let dir = tmp("size-relr");
    let cargo_target = dir.join("cargo");
    for target in ["llvm", "rust"] {
        for release in [false, true] {
            let profile = if release { "release" } else { "dev" };
            let out = dir.join(format!("{target}-{profile}"));
            let executable = build(target, release, &out, &cargo_target);
            let dynamic = readelf("-dW", &executable);
            assert!(
                dynamic.contains("(RELR)"),
                "{target} {profile}: {} has no DT_RELR:\n{dynamic}",
                executable.display()
            );
            let versions = readelf("-VW", &executable);
            assert!(
                versions.contains("GLIBC_ABI_DT_RELR"),
                "{target} {profile}: {} needs no GLIBC_ABI_DT_RELR:\n{versions}",
                executable.display()
            );
        }
    }
}

/// The symbol table `.symtab` of `file`: its entries as (type, binding,
/// name).
fn symtab(file: &Path) -> Vec<(String, String, String)> {
    let listed = readelf("-sW", file);
    let Some((_, table)) = listed.split_once("Symbol table '.symtab'") else {
        return Vec::new();
    };
    table
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            let number = fields.first()?.strip_suffix(':')?;
            number.parse::<usize>().ok()?;
            let name = fields.get(7).copied().unwrap_or_default();
            Some((fields[3].into(), fields[4].into(), name.into()))
        })
        .collect()
}

/// Row 3.2: an LLVM release executable keeps its global symbols, the
/// names a profiler or a debugger shows for the runtime's functions, and
/// drops what is local in the objects it links (`-Wl,--discard-all`): the
/// source file symbols and the exception table labels among them, which a
/// dev executable keeps; about a tenth of `todod`.
#[test]
fn an_llvm_release_executable_drops_the_local_symbols_of_its_objects() {
    let dir = tmp("size-locals");
    let cargo_target = dir.join("cargo");
    let count = |symbols: &[(String, String, String)]| {
        let files = symbols.iter().filter(|(kind, _, _)| kind == "FILE").count();
        let labels = symbols
            .iter()
            .filter(|(_, _, name)| name.starts_with("GCC_except_table"))
            .count();
        let globals = symbols
            .iter()
            .filter(|(kind, bind, _)| kind == "FUNC" && bind == "GLOBAL")
            .count();
        (files, labels, globals)
    };
    let release = build("llvm", true, &dir.join("release"), &cargo_target);
    let (files, labels, globals) = count(&symtab(&release));
    assert_eq!(
        (files, labels),
        (0, 0),
        "{} keeps local symbols of its objects",
        release.display()
    );
    assert!(
        globals > 0,
        "{} keeps no global function",
        release.display()
    );
    let dev = build("llvm", false, &dir.join("dev"), &cargo_target);
    let (files, labels, _) = count(&symtab(&dev));
    assert!(
        files > 0 && labels > 0,
        "{} lost its local symbols",
        dev.display()
    );
}

/// A release executable reports a failure as the dev one does, on both
/// targets: the same message, naming the Ignite source's file, line and
/// column, and the same exit status. The release links and the release
/// runtime of story `binary-size-back` change what an executable carries,
/// not what it reports.
#[test]
fn a_release_failure_report_is_the_dev_one_on_both_targets() {
    let dir = tmp("size-report");
    let cargo_target = dir.join("cargo");
    for target in ["llvm", "rust"] {
        let reports: Vec<(Option<i32>, String)> = [false, true]
            .into_iter()
            .map(|release| {
                let profile = if release { "release" } else { "dev" };
                let out = dir.join(format!("{target}-{profile}"));
                let executable = build(target, release, &out, &cargo_target);
                let output = Command::new(&executable)
                    .env_remove("RUST_BACKTRACE")
                    .env_remove("RUST_LIB_BACKTRACE")
                    .output()
                    .expect("run the executable");
                (output.status.code(), text(&output.stderr))
            })
            .collect();
        assert!(
            reports[0].1.contains("backtrace.ignite:12:10"),
            "{target} dev: {:?}",
            reports[0]
        );
        assert_eq!(reports[1], reports[0], "{target}: release against dev");
    }
}

/// The names by which the TLS engine's crates show in a symbol or a
/// compile unit: `rustls`, `rustls-webpki`, `rustls-pki-types`, `ring`
/// (its C objects' symbols start `ring_core_`) and `untrusted`.
const TLS_ENGINE: &[&str] = &["rustls", "webpki", "ring_core_", "/ring-", "untrusted"];

/// The compile units of `file`: the name of each top-level DIE.
fn compile_units(file: &Path) -> Vec<String> {
    let listed = text(
        &run(Command::new("readelf")
            .args(["--debug-dump=info", "--dwarf-depth=1"])
            .arg(file))
        .stdout,
    );
    listed
        .lines()
        .filter(|line| line.contains("DW_AT_name"))
        .map(|line| line.rsplit(": ").next().unwrap_or(line).to_string())
        .collect()
}

/// A program that names no `Tls` links no object of the TLS engine on the
/// LLVM target, dev and release (`docs/slice.md` 38.2, story
/// `binary-size-back`): no symbol and no compile unit of it, C objects
/// included. `tls::` builds and runs a program that opens a TLS session.
#[test]
fn an_llvm_program_without_tls_links_no_tls_engine_in_dev_and_release() {
    let dir = tmp("size-no-tls");
    let cargo_target = dir.join("cargo");
    for release in [false, true] {
        let profile = if release { "release" } else { "dev" };
        let executable = build("llvm", release, &dir.join(profile), &cargo_target);
        let listed = text(&run(Command::new("nm").arg("--defined-only").arg(&executable)).stdout);
        let symbols: Vec<&str> = listed
            .lines()
            .filter(|line| TLS_ENGINE.iter().any(|name| line.contains(name)))
            .collect();
        assert!(symbols.is_empty(), "{profile}: {symbols:#?}");
        let units: Vec<String> = compile_units(&executable)
            .into_iter()
            .filter(|unit| TLS_ENGINE.iter().any(|name| unit.contains(name)))
            .collect();
        assert!(units.is_empty(), "{profile}: {units:#?}");
        if !release {
            assert!(
                !compile_units(&executable).is_empty(),
                "{profile}: {} has no compile unit to check",
                executable.display()
            );
        }
    }
}

/// Row 3.8: an LLVM executable needs only the shared libraries it calls
/// (`-Wl,--as-needed` before the runtime's libraries, as rustc links the
/// Rust target): one that uses no math function does not load `libm`,
/// whose mapping is resident memory of every process.
#[test]
fn an_llvm_executable_loads_no_library_it_does_not_call() {
    let dir = tmp("size-as-needed");
    let cargo_target = dir.join("cargo");
    for release in [false, true] {
        let profile = if release { "release" } else { "dev" };
        let executable = build("llvm", release, &dir.join(profile), &cargo_target);
        let needed: Vec<String> = readelf("-dW", &executable)
            .lines()
            .filter(|line| line.contains("(NEEDED)"))
            .map(str::to_string)
            .collect();
        assert!(
            needed.iter().any(|line| line.contains("libc.so")),
            "{profile}: {needed:#?}"
        );
        assert!(
            !needed.iter().any(|line| line.contains("libm.so")),
            "{profile}: {needed:#?}"
        );
    }
}
