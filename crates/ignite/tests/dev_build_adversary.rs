//! Adversary pass on story `build-time-back` (row 6.2 rust of
//! `docs/quality.md`): the dependency build the driver starts ahead
//! (`crates/ignite/src/prebuild.rs`) is stopped when the driver fails, "so
//! none outlives the driver". A driver that is ended by a signal fails
//! nothing and drops nothing: this pins what is left running then. And the
//! debug information of the TLS engine's C code in a dev executable of the
//! Rust target, for a program built outside the repository.

use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The processes whose command line names `path`, each with its id.
fn processes_naming(path: &Path) -> Vec<(u32, String)> {
    let needle = path.to_string_lossy().into_owned();
    std::fs::read_dir("/proc")
        .unwrap()
        .flatten()
        .filter_map(|entry| {
            let id = entry.file_name().to_string_lossy().parse::<u32>().ok()?;
            let cmdline = std::fs::read(entry.path().join("cmdline")).ok()?;
            Some((id, String::from_utf8_lossy(&cmdline).replace('\0', " ")))
        })
        .filter(|(_, cmdline)| cmdline.contains(&needle))
        .collect()
}

/// `kill -<signal> <id>`.
fn signal(id: u32, signal: &str) {
    let _ = Command::new("kill")
        .arg(format!("-{signal}"))
        .arg(id.to_string())
        .stderr(Stdio::null())
        .status();
}

/// `kill <driver>` (SIGTERM, what a supervisor or a script sends to the one
/// process it started) while the driver still parses and generates: the
/// cargo it started ahead, and the compilers under it, are gone with it. A
/// second is allowed for them to end; the build ahead compiles `libc` and
/// `nix` without a compiler cache here, which takes several times that.
#[test]
fn a_driver_ended_by_sigterm_leaves_no_build_ahead() {
    a_driver_ended_by_leaves_no_build_ahead("TERM", 15);
}

/// The same with SIGINT, what a terminal sends on Ctrl-C (here to the
/// driver alone, not to its group).
#[test]
fn a_driver_ended_by_sigint_leaves_no_build_ahead() {
    a_driver_ended_by_leaves_no_build_ahead("INT", 2);
}

/// The same with SIGHUP, what a closed terminal sends.
#[test]
fn a_driver_ended_by_sighup_leaves_no_build_ahead() {
    a_driver_ended_by_leaves_no_build_ahead("HUP", 1);
}

/// `kill -<name> <driver>` while the driver still parses and generates,
/// the driver started with the signal's default action (`env
/// --default-signal`, whatever this test process ignores): the driver
/// ends by that signal and nothing of the build ahead runs a second later.
fn a_driver_ended_by_leaves_no_build_ahead(name: &str, number: i32) {
    let dir = tmp(&format!("ahead-killed-{name}"));
    let target = dir.join("cargo");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/todo/todo.ignite");
    let mut driver = Command::new("env")
        .arg("--default-signal")
        .arg(env!("CARGO_BIN_EXE_ignite"))
        .arg("build")
        .arg(&source)
        .args(["--target", "rust"])
        .arg("--out")
        .arg(dir.join("out"))
        .arg("--target-dir")
        .arg(&target)
        .env("RUSTC_WRAPPER", "")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let ahead = |driver: u32| -> Vec<(u32, String)> {
        processes_naming(&target)
            .into_iter()
            .filter(|(id, _)| *id != driver)
            .collect()
    };
    // The project is written when generation is over; a cargo seen before
    // that is the one started ahead, not the program's build.
    let project = dir.join("out/Cargo.toml");
    let mut started = false;
    for _ in 0..4000 {
        if project.exists() || driver.try_wait().unwrap().is_some() {
            break;
        }
        if !ahead(driver.id()).is_empty() {
            started = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    if !started {
        let _ = driver.kill();
        let _ = driver.wait();
        for (id, _) in ahead(0) {
            signal(id, "KILL");
        }
        panic!("the driver started no build ahead while it generated");
    }
    signal(driver.id(), name);
    let status = driver.wait().unwrap();
    let mut left = ahead(0);
    for _ in 0..20 {
        if left.is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
        left = ahead(0);
    }
    // Nothing this test started outlives it, whatever it found.
    for _ in 0..200 {
        let running = ahead(0);
        if running.is_empty() {
            break;
        }
        for (id, _) in &running {
            signal(*id, "KILL");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(
        status.signal(),
        Some(number),
        "the driver was to end by SIG{name}: {status:?}"
    );
    assert!(
        left.is_empty(),
        "{} processes of the build ahead still ran a second after the driver was gone: {left:#?}",
        left.len()
    );
}

/// The names of the compile units of `file` that lie in a cargo registry
/// or a git checkout (`readelf --debug-dump=info`, units only).
fn third_party_units(file: &Path) -> Vec<String> {
    let dump = Command::new("readelf")
        .args(["--wide", "--debug-dump=info", "--dwarf-depth=1"])
        .arg(file)
        .output()
        .unwrap();
    assert!(dump.status.success(), "readelf {}", file.display());
    String::from_utf8_lossy(&dump.stdout)
        .lines()
        .filter(|line| line.contains("DW_AT_name"))
        .filter(|line| line.contains("/registry/src/") || line.contains("/git/checkouts/"))
        .map(|line| line.rsplit(": ").next().unwrap_or(line).trim().to_string())
        .collect()
}

/// The story's acceptance for the Rust target: the generated project
/// builds its dependencies without debug information in the dev profile.
/// The C and assembly of the TLS engine get theirs switched off by the
/// repository's `.cargo/config.toml` (`CFLAGS = "-g0"`), which cargo reads
/// only when it runs inside the repository. A program is built where its
/// author keeps it: the driver runs in `/` here, the output lies where
/// every test's does.
#[test]
fn a_tls_program_built_outside_the_repository_holds_no_third_party_debug_information() {
    let dir = tmp("tls-outside");
    let source = dir.join("dial.ignite");
    std::fs::write(
        &source,
        "package dial\n\nimport std.tls.{TlsOptions}\n\n\
         fn open(port: Int) -> Result<TlsStream, IoError> {\n  \
         let stream = TcpStream.connect(\"127.0.0.1\", port)?\n  \
         Tls.client(stream, \"localhost\", TlsOptions {})\n}\n\n\
         type Dial {\n  executable: Text\n}\n\n\
         fn Dial.main(self: Dial) -> Result<Unit, IoError> {\n  \
         let session = open(9)?\n  session.close()\n}\n\n\
         system Dialer {\n  component dial: Dial { executable = \"dial\" }\n}\n",
    )
    .unwrap();
    let target = dir.join("cargo");
    let output = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("build")
        .arg(&source)
        .args(["--target", "rust"])
        .arg("--out")
        .arg(dir.join("out"))
        .arg("--target-dir")
        .arg(&target)
        .current_dir("/")
        // `cargo test` gives this process the repository's `[env]`, the
        // `CFLAGS` in question; a build outside the repository has none.
        .env_remove("CFLAGS")
        .env("RUSTC_WRAPPER", "")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let dial = target.join("debug/dial");
    let third = third_party_units(&dial);
    assert!(
        third.is_empty(),
        "{} compile units of third-party crates in {}, e.g. {:?}",
        third.len(),
        dial.display(),
        &third[..third.len().min(4)]
    );
}
