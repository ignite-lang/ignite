//! The LLVM driver's build steps (`docs/llvm.md` section 1): executables
//! are linked with unused sections dropped, an unchanged rebuild runs no
//! clang, an edit recompiles only the module it changed, a failed step runs
//! again, and the default runtime lookup.
//! `bench` (ignored, `task bench`) prints sizes and build times of both
//! targets.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Instant;

mod common;

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn terminal_source() -> PathBuf {
    workspace().join("examples/terminal/terminal.telos")
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

fn cargo() -> PathBuf {
    std::env::var_os("CARGO")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("cargo"))
}

fn run(command: &mut Command) -> Output {
    let output = command.output().expect("start the command");
    assert!(
        output.status.success(),
        "{command:?}\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

/// `telos build --target llvm` of `source` into `out`.
fn llvm_build(source: &Path, out: &Path, clang: &Path, release: bool) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_telos"));
    command
        .args(["build", "--target", "llvm"])
        .arg(source)
        .arg("--out")
        .arg(out)
        .arg("--clang")
        .arg(clang)
        .arg("--runtime")
        .arg(common::runtime_archive());
    if release {
        command.arg("--release");
    }
    command
}

/// A clang wrapper that appends each call's arguments to `log`, one line.
fn logging_clang(dir: &Path, log: &Path) -> PathBuf {
    let wrapper = dir.join("clang");
    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nexec '{}' \"$@\"\n",
        log.display(),
        which("clang").display()
    );
    std::fs::write(&wrapper, script).unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    wrapper
}

/// The clang calls logged since the last call: (compiled `.ll` file names,
/// number of links). Empties the log.
fn calls(log: &Path) -> (Vec<String>, usize) {
    let text = std::fs::read_to_string(log).unwrap_or_default();
    let _ = std::fs::remove_file(log);
    let mut compiled = Vec::new();
    let mut links = 0;
    for line in text.lines() {
        if line.split(' ').any(|arg| arg == "-c") {
            let ll = line
                .split(' ')
                .find(|arg| arg.ends_with(".ll"))
                .expect("a .ll input");
            compiled.push(Path::new(ll).file_name().unwrap().to_string_lossy().into());
        } else {
            assert!(line.contains("-Wl,--gc-sections"), "link line: {line}");
            links += 1;
        }
    }
    compiled.sort();
    (compiled, links)
}

/// `docs/llvm.md` section 1, object cache: an unchanged rebuild calls clang
/// zero times; an edit to the program recompiles `program.ll` only and
/// relinks both executables.
#[test]
fn rebuild_compiles_only_what_changed() {
    let dir = tmp("llvm-cache");
    let source = dir.join("terminal.telos");
    std::fs::copy(terminal_source(), &source).unwrap();
    let log = dir.join("clang.log");
    let clang = logging_clang(&dir, &log);
    let out = dir.join("out");

    run(&mut llvm_build(&source, &out, &clang, false));
    let (compiled, links) = calls(&log);
    assert_eq!(
        compiled,
        ["main_terminald.ll", "main_tn.ll", "program.ll", "std.ll"]
    );
    assert_eq!(links, 2);

    let output = run(&mut llvm_build(&source, &out, &clang, false));
    assert_eq!(calls(&log), (vec![], 0), "an unchanged rebuild ran clang");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!(
            "built {}\nbuilt {}\n",
            out.join("bin/terminald").display(),
            out.join("bin/tn").display()
        )
    );

    let text = std::fs::read_to_string(&source).unwrap();
    let edited = text.replace("lines = [\"ready\"]", "lines = [\"ready now\"]");
    assert_ne!(text, edited, "the edit did not apply");
    std::fs::write(&source, edited).unwrap();
    run(&mut llvm_build(&source, &out, &clang, false));
    assert_eq!(calls(&log), (vec!["program.ll".to_string()], 2));

    run(&mut llvm_build(&source, &out, &clang, true));
    let (compiled, links) = calls(&log);
    assert_eq!(compiled.len(), 4, "--release recompiles every module");
    assert_eq!(links, 2);
}

/// A failed compile leaves no stamp: the next build compiles again.
#[test]
fn failed_compile_is_retried() {
    let dir = tmp("llvm-cache-failure");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/probe.telos");
    let log = dir.join("clang.log");
    let clang = logging_clang(&dir, &log);
    let out = dir.join("out");
    run(&mut llvm_build(&source, &out, &clang, false));
    calls(&log);
    let failing = llvm_build(&source, &out, Path::new("/bin/false"), false)
        .output()
        .unwrap();
    assert_eq!(failing.status.code(), Some(2));
    assert!(!out.join("llvm/std.o.stamp").exists());
    run(&mut llvm_build(&source, &out, &clang, false));
    assert_eq!(calls(&log), (vec!["std.ll".to_string()], 0));
}

/// `docs/llvm.md` section 1, runtime lookup: `libtelos_rt.a` next to the
/// driver, then in its parent directory, as for a driver in
/// `target/<profile>/deps`.
#[test]
fn default_runtime_is_next_to_the_driver_or_in_its_parent() {
    let dir = tmp("llvm-runtime-lookup");
    let deps = dir.join("deps");
    std::fs::create_dir_all(&deps).unwrap();
    let driver = deps.join("telos");
    std::fs::copy(env!("CARGO_BIN_EXE_telos"), &driver).unwrap();
    assert_eq!(
        telos::runtime_candidates(&driver),
        [deps.join("libtelos_rt.a"), dir.join("libtelos_rt.a")]
    );
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/probe.telos");
    let build = || {
        Command::new(&driver)
            .args(["build", "--target", "llvm"])
            .arg(&source)
            .arg("--out")
            .arg(dir.join("out"))
            .output()
            .unwrap()
    };

    let missing = build();
    assert_eq!(missing.status.code(), Some(3));
    assert!(
        String::from_utf8_lossy(&missing.stderr)
            .contains(&*deps.join("libtelos_rt.a").to_string_lossy()),
        "{}",
        String::from_utf8_lossy(&missing.stderr)
    );

    std::fs::copy(common::runtime_archive(), dir.join("libtelos_rt.a")).unwrap();
    let parent = build();
    assert!(
        parent.status.success(),
        "{}",
        String::from_utf8_lossy(&parent.stderr)
    );

    std::fs::write(deps.join("libtelos_rt.a"), "not an archive").unwrap();
    let next_to = build();
    assert_eq!(
        next_to.status.code(),
        Some(2),
        "the archive next to the driver comes first"
    );
}

/// Seconds `f` takes.
fn time(f: impl FnOnce()) -> f64 {
    let start = Instant::now();
    f();
    start.elapsed().as_secs_f64()
}

fn size(file: &Path) -> u64 {
    std::fs::metadata(file).unwrap().len()
}

fn stripped_size(file: &Path, scratch: &Path) -> u64 {
    let stripped = scratch.join("stripped");
    run(Command::new("strip").arg("-o").arg(&stripped).arg(file));
    size(&stripped)
}

fn megabytes(bytes: u64) -> String {
    format!("{:.2} MB", bytes as f64 / 1e6)
}

/// A command with sccache off, so that a cold build is cold.
fn uncached(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut command = Command::new(program);
    command
        .env("RUSTC_WRAPPER", "")
        .env("CARGO_BUILD_RUSTC_WRAPPER", "")
        .env("CARGO_INCREMENTAL", "0");
    command
}

/// `task bench`: sizes of `terminald` for both targets and profiles, build
/// times (runtime archive and program cold, unchanged rebuild; no sccache)
/// and the time of 200 runs of `terminald --port nope`.
#[test]
#[ignore = "benchmark; run with task bench"]
fn bench() {
    let root = tmp("bench");
    let telos = env!("CARGO_BIN_EXE_telos");
    let source = terminal_source();
    let clang = which("clang");
    let profiles = ["debug", "release"];
    let mut rows: Vec<(String, Vec<String>)> = Vec::new();
    let mut sizes = Vec::new();
    let mut runtime_cold = Vec::new();
    let mut cold = Vec::new();
    let mut rebuild = Vec::new();
    let mut startup = Vec::new();
    let mut binaries = Vec::new();

    for profile in profiles {
        let release = profile == "release";
        let rt_target = root.join("runtime");
        let mut command = uncached(cargo());
        command
            .args(["build", "-q", "-p", "telos-rt", "--manifest-path"])
            .arg(workspace().join("Cargo.toml"))
            .arg("--target-dir")
            .arg(&rt_target);
        if release {
            command.arg("--release");
        }
        runtime_cold.push(time(|| {
            run(&mut command);
        }));
        let runtime = rt_target.join(profile).join("libtelos_rt.a");
        let out = root.join(format!("llvm-{profile}"));
        let mut build = Command::new(telos);
        build
            .args(["build", "--target", "llvm"])
            .arg(&source)
            .arg("--out")
            .arg(&out)
            .arg("--clang")
            .arg(&clang)
            .arg("--runtime")
            .arg(&runtime);
        if release {
            build.arg("--release");
        }
        cold.push(time(|| {
            run(&mut build);
        }));
        rebuild.push(time(|| {
            run(&mut build);
        }));
        binaries.push(out.join("bin/terminald"));
    }
    for profile in profiles {
        let out = root.join(format!("rust-{profile}"));
        let target_dir = root.join(format!("rust-{profile}-target"));
        let mut build = uncached(telos);
        build
            .arg("build")
            .arg(&source)
            .arg("--out")
            .arg(&out)
            .arg("--target-dir")
            .arg(&target_dir);
        if profile == "release" {
            build.arg("--release");
        }
        cold.push(time(|| {
            run(&mut build);
        }));
        rebuild.push(time(|| {
            run(&mut build);
        }));
        binaries.push(target_dir.join(profile).join("terminald"));
    }
    for binary in &binaries {
        sizes.push((size(binary), stripped_size(binary, &root)));
        startup.push(time(|| {
            for _ in 0..200 {
                let status = Command::new(binary)
                    .args(["--port", "nope"])
                    .output()
                    .unwrap()
                    .status;
                assert_eq!(status.code(), Some(2));
            }
        }));
    }

    let seconds =
        |values: &[f64]| -> Vec<String> { values.iter().map(|s| format!("{s:.2} s")).collect() };
    rows.push((
        "terminald size".into(),
        sizes.iter().map(|(full, _)| megabytes(*full)).collect(),
    ));
    rows.push((
        "terminald stripped".into(),
        sizes
            .iter()
            .map(|(_, stripped)| megabytes(*stripped))
            .collect(),
    ));
    rows.push((
        "runtime archive cold".into(),
        seconds(&runtime_cold)
            .into_iter()
            .chain(["-".into(), "-".into()])
            .collect(),
    ));
    rows.push(("program build cold".into(), seconds(&cold)));
    rows.push(("rebuild unchanged".into(), seconds(&rebuild)));
    rows.push(("200 runs --port nope".into(), seconds(&startup)));
    println!(
        "\n{:<22}{:>14}{:>14}{:>14}{:>14}",
        "", "llvm debug", "llvm release", "rust debug", "rust release"
    );
    for (label, values) in rows {
        print!("{label:<22}");
        for value in values {
            print!("{value:>14}");
        }
        println!();
    }
}
