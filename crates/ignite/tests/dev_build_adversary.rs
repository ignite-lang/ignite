//! Adversary pass on story `build-time-back` (row 6.2 rust of
//! `docs/quality.md`): the dependency build the driver starts ahead
//! (`crates/ignite/src/prebuild.rs`) is stopped when the driver fails, "so
//! none outlives the driver". A driver that is ended by a signal fails
//! nothing and drops nothing: this pins what is left running then.

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
    let dir = tmp("ahead-killed");
    let target = dir.join("cargo");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/todo/todo.ignite");
    let mut driver = Command::new(env!("CARGO_BIN_EXE_ignite"))
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
    signal(driver.id(), "TERM");
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
        Some(15),
        "the driver was to end by SIGTERM: {status:?}"
    );
    assert!(
        left.is_empty(),
        "{} processes of the build ahead still ran a second after the driver was gone: {left:#?}",
        left.len()
    );
}
