//! Adversary cases of `story:binary-size-back`, Rust target. The generated
//! project's dev profile builds `ignite-std` without debug information
//! (`STD_DEV_PROFILE` in `crates/ignite-rustgen/src/lib.rs`), and the
//! story's acceptance keeps "a dev backtrace of a failing Ignite program
//! still names file and line": `dev_build`'s backtrace fixture fails in
//! the program's own fn, the one here in a fn of the standard library.
//! And the driver's link argument for packed relocations reaches cargo as
//! a target's `rustflags`, which takes the place of the `build.rustflags`
//! of the user's cargo configuration.

use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// `gdb` runs `executable` to the failure report of `ignite-io` and prints
/// the backtrace there, as `dev_build::gdb_backtrace`.
fn gdb_backtrace(executable: &Path) -> String {
    let output = Command::new("gdb")
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

/// The link argument the driver gives a dynamic Rust build goes to cargo as
/// `target.'cfg(all())'.rustflags` (`link_rustflags` in `src/lib.rs`).
/// Cargo takes its extra flags from one source only, the first of
/// `RUSTFLAGS`, the matching `target.*.rustflags` joined, and
/// `build.rustflags`: with a target entry always there, the
/// `build.rustflags` of the user's cargo configuration (a processor to
/// build for, a linker) no longer reach the build, which they did before.
/// Here `build.rustflags` (as `CARGO_BUILD_RUSTFLAGS`) names a run path,
/// which the executable's dynamic section must then hold. The cargo home
/// is one of this test's own (the registry of the real one linked in) and
/// the build runs from `/`, so that no other cargo configuration, with
/// target flags of its own, takes part.
#[test]
fn a_rust_build_keeps_the_build_rustflags_of_the_cargo_configuration() {
    const MARKER: &str = "/build-rustflags-marker";
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/backtrace.ignite");
    let dir = tmp("size-build-rustflags");
    let real_home = std::env::var_os("CARGO_HOME").map_or_else(
        || Path::new(&std::env::var_os("HOME").expect("HOME")).join(".cargo"),
        PathBuf::from,
    );
    let home = dir.join("cargo-home");
    std::fs::create_dir_all(&home).unwrap();
    std::os::unix::fs::symlink(real_home.join("registry"), home.join("registry")).unwrap();
    let target = dir.join("cargo");
    let build = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("build")
        .arg(&source)
        .args(["--target", "rust"])
        .arg("--out")
        .arg(dir.join("out"))
        .arg("--target-dir")
        .arg(&target)
        .current_dir("/")
        .env("CARGO_HOME", &home)
        .env(
            "CARGO_BUILD_RUSTFLAGS",
            format!("-C link-arg=-Wl,-rpath,{MARKER}"),
        )
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS")
        .output()
        .expect("start ignite build");
    assert!(
        build.status.success(),
        "ignite build: {}{}",
        text(&build.stdout),
        text(&build.stderr)
    );
    let dynamic = Command::new("readelf")
        .arg("-dW")
        .arg(target.join("debug/overflow"))
        .output()
        .expect("start readelf");
    let dynamic = text(&dynamic.stdout);
    assert!(
        dynamic.contains(MARKER),
        "the run path of build.rustflags is not in the executable:\n{dynamic}"
    );
}

/// Adversary pass 2: since `ece12a40` a dynamic Rust build passes no
/// `rustflags` of its own. The case above builds in dev with
/// `build.rustflags`; these are the other ways a user's flags reach rustc
/// and the other profile: `RUSTFLAGS` in a dev build and `build.rustflags`
/// in a release build each put their run path into the executable. The
/// release build also writes no split debug information: the `.dwo` files
/// are the dev profile's (`dev_member_profile` in
/// `crates/ignite-rustgen/src/lib.rs`), and the executable has no debug
/// section.
#[test]
fn a_rust_build_takes_the_users_rustflags_in_dev_and_release() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/backtrace.ignite");
    let real_home = std::env::var_os("CARGO_HOME").map_or_else(
        || Path::new(&std::env::var_os("HOME").expect("HOME")).join(".cargo"),
        PathBuf::from,
    );
    for (variable, release) in [("RUSTFLAGS", false), ("CARGO_BUILD_RUSTFLAGS", true)] {
        let marker = format!("/rustflags-marker-{}", variable.to_lowercase());
        let profile = if release { "release" } else { "debug" };
        let dir = tmp(&format!("size-user-rustflags-{profile}"));
        let home = dir.join("cargo-home");
        std::fs::create_dir_all(&home).unwrap();
        std::os::unix::fs::symlink(real_home.join("registry"), home.join("registry")).unwrap();
        let target = dir.join("cargo");
        let mut command = Command::new(env!("CARGO_BIN_EXE_ignite"));
        command
            .arg("build")
            .arg(&source)
            .args(["--target", "rust"])
            .arg("--out")
            .arg(dir.join("out"))
            .arg("--target-dir")
            .arg(&target)
            .current_dir("/")
            .env("CARGO_HOME", &home)
            .env_remove("RUSTFLAGS")
            .env_remove("CARGO_BUILD_RUSTFLAGS")
            .env_remove("CARGO_ENCODED_RUSTFLAGS")
            .env_remove("CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS")
            .env(variable, format!("-C link-arg=-Wl,-rpath,{marker}"));
        if release {
            command.arg("--release");
        }
        let build = command.output().expect("start ignite build");
        assert!(
            build.status.success(),
            "ignite build ({variable}, {profile}): {}{}",
            text(&build.stdout),
            text(&build.stderr)
        );
        let executable = target.join(profile).join("overflow");
        let dynamic = Command::new("readelf")
            .arg("-dW")
            .arg(&executable)
            .output()
            .expect("start readelf");
        let dynamic = text(&dynamic.stdout);
        assert!(
            dynamic.contains(&marker),
            "the run path of {variable} is not in the {profile} executable:\n{dynamic}"
        );
        if release {
            let split: Vec<PathBuf> = std::fs::read_dir(target.join("release/deps"))
                .expect("the release build's deps")
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "dwo"))
                .collect();
            assert!(
                split.is_empty(),
                "a release build wrote split debug information: {split:?}"
            );
            let sections = Command::new("readelf")
                .arg("-SW")
                .arg(&executable)
                .output()
                .expect("start readelf");
            let sections = text(&sections.stdout);
            assert!(
                !sections.contains(".debug_"),
                "the release executable has debug sections:\n{sections}"
            );
        }
    }
}

/// A program that fails inside the standard library (`std.time`'s
/// `days_from_civil` multiplies past the range of `Int`), built in dev
/// for the Rust target: the backtrace under `gdb` has the frame of
/// `ignite_std::time::days_from_civil`, and that frame names a file and a
/// line, as it did before the generated `ignite-std` lost its line tables
/// (`std/src/lib.rs:<line>`), and as the program's own frame beside it
/// still does.
#[test]
fn a_rust_dev_backtrace_names_file_and_line_of_a_standard_library_frame() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/backtrace_std.ignite");
    let out = tmp("dev-backtrace-std");
    let target = Path::new(env!("CARGO_TARGET_TMPDIR")).join("dev-backtrace-std-cargo");
    let build = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("build")
        .arg(&source)
        .args(["--target", "rust"])
        .arg("--out")
        .arg(&out)
        .arg("--target-dir")
        .arg(&target)
        .output()
        .expect("start ignite build");
    assert!(
        build.status.success(),
        "ignite build: {}{}",
        text(&build.stdout),
        text(&build.stderr)
    );
    let backtrace = gdb_backtrace(&target.join("debug/civil"));
    let frames: Vec<&str> = backtrace
        .lines()
        .filter(|line| line.starts_with('#'))
        .collect();
    let program = frames
        .iter()
        .find(|line| line.contains("Civil__main"))
        .unwrap_or_else(|| panic!("no frame of the program's main:\n{backtrace}"));
    assert!(
        program.contains(" at src/lib.rs:"),
        "the program's frame names no file and line: {program}\n{backtrace}"
    );
    let library = frames
        .iter()
        .find(|line| line.contains("ignite_std::time::days_from_civil"))
        .unwrap_or_else(|| panic!("no frame of days_from_civil:\n{backtrace}"));
    let named = library.rsplit_once(" at ").is_some_and(|(_, at)| {
        at.rsplit_once(':')
            .is_some_and(|(file, line)| !file.is_empty() && line.trim().parse::<usize>().is_ok())
    });
    assert!(
        named,
        "the standard library's frame names no file and line: {library}\n{}",
        frames.join("\n")
    );
}
