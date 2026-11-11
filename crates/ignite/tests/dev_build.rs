//! What a dev build costs and keeps (story `build-time-back`, rows 6.2 and
//! 3.2 of `docs/quality.md`): the dev runtime archive carries no debug
//! information of third-party crates and line tables only for `ignite-rt`
//! and `ignite-io`, whatever the dev profile's `debug` (the dedicated host
//! has no cargo configuration, so its default is full debug information);
//! a dev link on the LLVM target uses lld when it lies beside clang and
//! the default linker otherwise, a build id either way; a fresh build on
//! the Rust target resolves no dependency over the network; and a dev
//! backtrace still names the Ignite source and the substrate's frames
//! with file and line on both targets.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crate::common;

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn which(tool: &str) -> PathBuf {
    std::env::var_os("PATH")
        .and_then(|paths| {
            std::env::split_paths(&paths)
                .map(|dir| dir.join(tool))
                .find(|path| path.is_file())
        })
        .unwrap_or_else(|| panic!("{tool} not on PATH"))
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

/// `ignite build <source> --target <target> --out <out>` in dev.
fn ignite_build(source: &Path, target: &str, out: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ignite"));
    command
        .arg("build")
        .arg(source)
        .args(["--target", target])
        .arg("--out")
        .arg(out);
    command
}

/// One compile unit of an executable's DWARF: its name, its compilation
/// directory and the tags of its direct children.
#[derive(Debug)]
struct Unit {
    name: String,
    dir: String,
    children: Vec<String>,
}

impl Unit {
    /// Built from a crate of a registry or a git checkout: neither the
    /// workspace, nor the program, nor the Rust standard library.
    fn third_party(&self) -> bool {
        [&self.name, &self.dir]
            .iter()
            .any(|path| path.contains("/registry/src/") || path.contains("/git/checkouts/"))
    }

    /// A unit of the Rust crate `krate`: rustc names it `<root>/@/<krate>.<hash>-cgu.<n>`.
    fn of_crate(&self, krate: &str) -> bool {
        self.name.contains(&format!("/@/{krate}."))
    }

    /// Whether the unit describes types or variables, which line tables
    /// alone never do.
    fn has_types(&self) -> bool {
        self.children.iter().any(|tag| {
            matches!(
                tag.as_str(),
                "DW_TAG_base_type"
                    | "DW_TAG_structure_type"
                    | "DW_TAG_pointer_type"
                    | "DW_TAG_enumeration_type"
                    | "DW_TAG_union_type"
                    | "DW_TAG_variable"
            )
        })
    }
}

/// The value of a `readelf --debug-dump=info` attribute line.
fn attribute(line: &str) -> String {
    let value = match line.rfind("): ") {
        Some(at) => &line[at + 3..],
        None => line.split_once(": ").map_or("", |(_, value)| value),
    };
    value.trim().to_string()
}

/// The compile units of `file` (`readelf --debug-dump=info`, units and
/// their direct children only).
fn compile_units(file: &Path) -> Vec<Unit> {
    let dump = run(Command::new("readelf")
        .args(["--wide", "--debug-dump=info", "--dwarf-depth=2"])
        .arg(file));
    let mut units: Vec<Unit> = Vec::new();
    let mut in_unit = false;
    for line in text(&dump.stdout).lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("<0>") {
            in_unit = line.contains("DW_TAG_compile_unit") || line.contains("DW_TAG_skeleton_unit");
            if in_unit {
                units.push(Unit {
                    name: String::new(),
                    dir: String::new(),
                    children: Vec::new(),
                });
            }
        } else if trimmed.starts_with("<1>") {
            in_unit = false;
            if let (Some(unit), Some(tag)) = (
                units.last_mut(),
                line.rsplit_once('(')
                    .map(|(_, tag)| tag.trim_end_matches(')').to_string()),
            ) {
                unit.children.push(tag);
            }
        } else if in_unit && let Some(unit) = units.last_mut() {
            if trimmed.contains("DW_AT_name") {
                unit.name = attribute(trimmed);
            } else if trimmed.contains("DW_AT_comp_dir") {
                unit.dir = attribute(trimmed);
            }
        }
    }
    units
}

/// `libignite_rt.a` with the test runner, built in dev into `target_dir`
/// with the dev profile's `debug` set to `debug` (`None`: the configured
/// one).
fn dev_runtime(target_dir: &Path, debug: Option<&str>) -> PathBuf {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(cargo);
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
        .arg(workspace().join("Cargo.toml"))
        .arg("--target-dir")
        .arg(target_dir)
        .env_remove("CARGO_TARGET_DIR")
        .env_remove("CARGO_BUILD_TARGET")
        .env_remove("RUSTC_WORKSPACE_WRAPPER");
    match debug {
        Some(debug) => command.env("CARGO_PROFILE_DEV_DEBUG", debug),
        None => command.env_remove("CARGO_PROFILE_DEV_DEBUG"),
    };
    run(&mut command);
    target_dir.join("debug/libignite_rt.a")
}

/// Story `build-time-back`, row 6.2 llvm: the dev `todod` on the LLVM
/// target, linked against a dev runtime archive built with the dev
/// profile's `debug` as configured and with `CARGO_PROFILE_DEV_DEBUG=2`
/// (the dedicated host's default), holds no compile unit of a
/// third-party crate, and the units of `ignite-rt` and `ignite-io` are
/// there, as line tables: the linker copies and relocates every debug
/// section it loads, and these were 80 % of the link's input.
#[test]
fn dev_todod_links_no_third_party_debug_information() {
    let runtimes = Path::new(env!("CARGO_TARGET_TMPDIR")).join("dev-runtime");
    for (variant, debug) in [("configured", None), ("full", Some("2"))] {
        let runtime = dev_runtime(&runtimes.join(variant), debug);
        let out = tmp(&format!("dev-todod-{variant}"));
        run(
            ignite_build(&workspace().join("examples/todo/todo.ignite"), "llvm", &out)
                .arg("--runtime")
                .arg(&runtime),
        );
        let todod = out.join("bin/todod");
        let units = compile_units(&todod);
        let third: Vec<&str> = units
            .iter()
            .filter(|unit| unit.third_party())
            .map(|unit| unit.name.as_str())
            .collect();
        assert!(
            third.is_empty(),
            "{variant}: {} of {} compile units in {} are third-party crates', e.g. {:?}",
            third.len(),
            units.len(),
            todod.display(),
            &third[..third.len().min(8)]
        );
        for krate in ["ignite_rt", "ignite_io"] {
            let own: Vec<&Unit> = units.iter().filter(|unit| unit.of_crate(krate)).collect();
            assert!(
                !own.is_empty(),
                "{variant}: no compile unit of {krate} in {}",
                todod.display()
            );
            let typed: Vec<&str> = own
                .iter()
                .filter(|unit| unit.has_types())
                .map(|unit| unit.name.as_str())
                .collect();
            assert!(
                typed.is_empty(),
                "{variant}: compile units of {krate} carry more than line tables: {typed:?}"
            );
        }
    }
}

/// A shell script named `clang` in `dir` that runs the clang on `PATH`.
fn clang_wrapper(dir: &Path) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let wrapper = dir.join("clang");
    let script = format!("#!/bin/sh\nexec '{}' \"$@\"\n", which("clang").display());
    std::fs::write(&wrapper, script).unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    wrapper
}

/// The `.comment` strings of an ELF file, where lld names itself.
fn comment(file: &Path) -> String {
    text(&run(Command::new("readelf").args(["-p", ".comment"]).arg(file)).stdout)
}

/// `docs/llvm.md` section 1, row 6.2 llvm: a dev build links with lld
/// when `ld.lld` lies beside the clang it runs (its symbolic links
/// resolved), and with clang's default linker otherwise, even with
/// `ld.lld` on `PATH`; both executables carry a build id.
#[test]
fn dev_link_uses_lld_beside_clang_else_the_default_linker() {
    let dir = tmp("dev-link-lld");
    let beside = clang_wrapper(&dir.join("with-lld"));
    std::os::unix::fs::symlink(which("ld.lld"), dir.join("with-lld/ld.lld")).unwrap();
    let alone = clang_wrapper(&dir.join("without-lld"));
    let source = workspace().join("examples/todo/todo.ignite");
    for (clang, lld) in [(&beside, true), (&alone, false)] {
        let out = dir.join(if lld { "out-lld" } else { "out-default" });
        run(ignite_build(&source, "llvm", &out)
            .arg("--clang")
            .arg(clang)
            .arg("--runtime")
            .arg(common::runtime_archive()));
        let todod = out.join("bin/todod");
        assert_eq!(
            comment(&todod).contains("Linker: LLD"),
            lld,
            "{} with {}: {}",
            todod.display(),
            clang.display(),
            comment(&todod)
        );
        let notes = text(&run(Command::new("readelf").arg("-n").arg(&todod)).stdout);
        assert!(
            notes.contains("Build ID"),
            "{} has no build id:\n{notes}",
            todod.display()
        );
    }
}

/// Row 6.2 rust: a build into a fresh `--out` starts from a lock file, so
/// cargo asks the registry nothing. Every request through the proxy given
/// here fails, which fails the build.
#[test]
fn fresh_rust_build_makes_no_index_request() {
    let dir = tmp("rust-no-index");
    let out = dir.join("out");
    let target = Path::new(env!("CARGO_TARGET_TMPDIR")).join("rust-no-index-cargo");
    let built = run(ignite_build(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/backtrace.ignite"),
        "rust",
        &out,
    )
    .arg("--target-dir")
    .arg(&target)
    .env("CARGO_HTTP_PROXY", "http://127.0.0.1:9")
    .env("CARGO_NET_RETRY", "0")
    .env("CARGO_HTTP_TIMEOUT", "10")
    .env_remove("CARGO_NET_OFFLINE"));
    let stderr = text(&built.stderr);
    assert!(
        !stderr.contains("Updating crates.io index"),
        "the build updated the index:\n{stderr}"
    );
    assert!(out.join("Cargo.lock").is_file());
}

/// The line of `add`'s `a + b` in the fixture.
fn overflow_line(source: &Path) -> usize {
    std::fs::read_to_string(source)
        .unwrap()
        .lines()
        .position(|line| line.contains("return a + b"))
        .expect("the fixture adds in `add`")
        + 1
}

/// `gdb` runs `executable` to the failure report of `ignite-io`, prints
/// the backtrace there and lets the process end.
fn gdb_backtrace(executable: &Path) -> String {
    let output = Command::new(which("gdb"))
        .args(["-nx", "-batch"])
        .args(["-ex", "set breakpoint pending on"])
        .args(["-ex", "break ignite_io::report::failure_report"])
        .args(["-ex", "run", "-ex", "bt", "-ex", "continue"])
        .arg("--args")
        .arg(executable)
        .env("DEBUGINFOD_URLS", "")
        .output()
        .expect("start gdb");
    format!("{}{}", text(&output.stdout), text(&output.stderr))
}

/// The frames of `backtrace` that name a file and a line (`#N [0x... in]
/// <fn> (<args>) at <file>:<line>`): the text before ` at `, the file, the
/// line.
fn frames(backtrace: &str) -> Vec<(&str, &str, usize)> {
    backtrace
        .lines()
        .filter(|line| line.starts_with('#'))
        .filter_map(|line| {
            let (head, at) = line.rsplit_once(" at ")?;
            let (file, number) = at.trim().rsplit_once(':')?;
            Some((head, file, number.parse().ok()?))
        })
        .collect()
}

/// Whether a frame of `backtrace` lies in a file whose path contains
/// `file`, with a line.
fn frame_in(backtrace: &str, file: &str) -> bool {
    frames(backtrace)
        .iter()
        .any(|(_, path, _)| path.contains(file))
}

/// Whether the innermost frame, the breakpoint's `failure_report`, which
/// `ignite-io` compiles itself, names its file (`src/report.rs`, relative
/// where cargo compiled the crate as a path dependency) and line.
fn breakpoint_frame_has_its_line(backtrace: &str) -> bool {
    backtrace
        .lines()
        .find(|line| line.starts_with("#0 "))
        .is_some_and(|line| frame_in(line, "src/report.rs"))
}

/// `docs/llvm.md` section 9: a dev backtrace of a failing program names
/// file and line of the substrate's frames and of the Ignite source: on
/// the LLVM target the Ignite fn's frame itself (its debug metadata), and
/// `ignite-rt`'s and `ignite-io`'s frames; on the Rust target
/// `ignite-io`'s frames and the generated program's, whose failure report
/// names the Ignite source. The substrate frames checked are of code its
/// crates compile themselves (`failure_report`, `ignite_rt_fail_arith`),
/// not generic instances a member compiles with its own line tables.
#[test]
fn dev_backtrace_names_ignite_and_substrate_lines_on_both_targets() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/backtrace.ignite");
    let line = overflow_line(&source);
    let dir = tmp("dev-backtrace");
    let llvm = dir.join("llvm");
    run(ignite_build(&source, "llvm", &llvm)
        .arg("--runtime")
        .arg(common::runtime_archive()));
    let backtrace = gdb_backtrace(&llvm.join("bin/overflow"));
    assert!(
        frames(&backtrace)
            .iter()
            .any(|(head, file, at)| head.contains("add")
                && file.ends_with("/backtrace.ignite")
                && *at == line),
        "no frame of `add` at backtrace.ignite:{line}:\n{backtrace}"
    );
    assert!(
        breakpoint_frame_has_its_line(&backtrace),
        "ignite-io's failure_report has no file and line:\n{backtrace}"
    );
    assert!(
        frame_in(&backtrace, "ignite-rt/src/fail.rs"),
        "no frame with file and line in ignite-rt/src/fail.rs:\n{backtrace}"
    );

    let rust = dir.join("rust");
    let target = Path::new(env!("CARGO_TARGET_TMPDIR")).join("dev-backtrace-cargo");
    run(ignite_build(&source, "rust", &rust)
        .arg("--target-dir")
        .arg(&target));
    let backtrace = gdb_backtrace(&target.join("debug/overflow"));
    assert!(
        breakpoint_frame_has_its_line(&backtrace),
        "no frame of ignite-io's own code with file and line:\n{backtrace}"
    );
    assert!(
        frames(&backtrace)
            .iter()
            .any(|(head, file, _)| head.contains("add")
                && file.ends_with("src/lib.rs")
                && !file.contains("ignite-io")),
        "no frame of the generated `add` with file and line:\n{backtrace}"
    );
    assert!(
        backtrace.contains(&format!("backtrace.ignite:{line}:")),
        "the failure report names no backtrace.ignite:{line}:\n{backtrace}"
    );
}
