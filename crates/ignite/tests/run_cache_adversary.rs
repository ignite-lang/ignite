//! IG-04 (`docs/slice.md` 16.3): a build of `ignite run` is reused only
//! for the same entry file with unchanged sources, and a run never starts
//! the executable of another entry file. Each case runs a copy of
//! `run_gave_up.ignite` whose member prints `first` or `second`, with
//! `--clock manual`, and reads what `<state>/bin/1/<executable>` prints.
//! The component is `adv_cache_daemon`, a name no other run test builds,
//! so the cargo target directory the Rust-target run tests share holds it
//! only from these cases.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::common;

const EXECUTABLE: &str = "adv_cache_daemon";
/// The longest a run may take to exit after `stop`.
const STOPPED: Duration = Duration::from_secs(30);

fn tmp(name: &str) -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("run-cache-adv-{name}"))
}

/// A fresh directory under the test target directory.
fn fresh(name: &str) -> PathBuf {
    let dir = tmp(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The cargo target directory every Rust-target run test shares
/// (`run.rs`), so the substrate is not compiled again.
fn shared_cargo_target_dir() -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join("run-cargo")
}

/// The text of `run_gave_up.ignite` with its own package and a member
/// that prints `word`.
fn program(word: &str) -> String {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/run_gave_up.ignite"),
    )
    .unwrap();
    let text = source
        .replace(
            "package fixture_run_gave_up",
            &format!("package fixture_run_cache_adv_{word}"),
        )
        .replace(
            "-> Result<Unit, IoError> = Ok(Unit)",
            &format!("-> Result<Unit, IoError> = Stdout.write_all(b\"{word}\\n\")"),
        )
        .replace("component daemon:", &format!("component {EXECUTABLE}:"))
        .replace("S.daemon =", &format!("S.{EXECUTABLE} ="));
    assert_eq!(text.matches(EXECUTABLE).count(), 2, "{text}");
    text
}

/// One `ignite run file --build build --clock manual` with a fresh state
/// named `name`, stopped with `stop` once its first segment is out; what
/// the copied `<state>/bin/1/adv_cache_daemon` prints.
fn run(file: &Path, build: &Path, target: &str, target_dir: Option<&Path>, name: &str) -> String {
    let state = tmp(&format!("state-{name}"));
    let _ = std::fs::remove_dir_all(&state);
    let mut command = Command::new(env!("CARGO_BIN_EXE_ignite"));
    command
        .arg("run")
        .arg(file)
        .args(["--assembly", "local", "--state"])
        .arg(&state)
        .arg("--build")
        .arg(build)
        .args(["--target", target, "--json", "--settle", "1 s"])
        .args(["--clock", "manual"]);
    if target == "llvm" {
        command.arg("--runtime").arg(common::runtime_archive());
    }
    if let Some(dir) = target_dir {
        command.arg("--target-dir").arg(dir);
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn ignite run");
    let mut stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let mut stderr = child.stderr.take().expect("stderr");
    let (tx, lines) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let errors = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text);
        text
    });
    let mut errors = Some(errors);
    let mut stderr_text = || {
        errors
            .take()
            .map(|handle| handle.join().unwrap_or_default())
            .unwrap_or_default()
    };
    let started = lines.recv_timeout(common::SETUP);
    if started.is_err() {
        let _ = child.kill();
        let _ = child.wait();
        panic!("{name}: no segment\nstderr:\n{}", stderr_text());
    }
    writeln!(stdin, "stop").expect("write stop");
    stdin.flush().expect("flush stop");
    let deadline = Instant::now() + STOPPED;
    loop {
        if child.try_wait().expect("wait for ignite run").is_some() {
            break;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("{name}: no exit after stop\nstderr:\n{}", stderr_text());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    drop(stdin);
    let daemon = state.join("bin/1").join(EXECUTABLE);
    let output = Command::new(&daemon)
        .output()
        .unwrap_or_else(|e| panic!("{}: {e}\nstderr:\n{}", daemon.display(), stderr_text()));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// The first line of `<build>/stamp`: the stamp of the last build.
fn stamp(build: &Path) -> String {
    std::fs::read_to_string(build.join("stamp"))
        .expect("read the build stamp")
        .lines()
        .next()
        .unwrap_or_default()
        .to_string()
}

/// 16.3: "Builds of the same entry file with unchanged sources are a no-op".
/// An entry file without the `.ignite` extension is still a run's entry
/// (`read_source` reads any path), but the stamp holds its canonical path
/// only and scans the `.ignite` files of its directory for contents. After
/// a build, a second program moved onto the entry's name keeps the path
/// and the stamp, and the run starts the first program's executable.
#[test]
fn an_entry_without_the_ignite_extension_replaced_after_a_build_runs_the_new_program() {
    let dir = fresh("noext");
    let build = tmp("noext-build");
    let _ = std::fs::remove_dir_all(&build);
    let entry = dir.join("daemon_entry");
    std::fs::write(&entry, program("first")).unwrap();
    assert_eq!(run(&entry, &build, "llvm", None, "noext-1"), "first\n");
    let next = dir.join("next_entry");
    std::fs::write(&next, program("second")).unwrap();
    std::fs::rename(&next, &entry).unwrap();
    assert_eq!(
        run(&entry, &build, "llvm", None, "noext-2"),
        "second\n",
        "the entry now holds the second program; the run started the first one's executable"
    );
}

/// `--target-dir` is a flag of `ignite run` (16.3) and the Rust target's
/// executables lie in it, not in `--build`; the run tests share one. Two
/// entry files, each with its own `--build`, through one target
/// directory: after the second entry built its `adv_cache_daemon` there,
/// the first entry's unchanged stamp still lists that path, and the run of
/// the first entry starts the second entry's executable.
#[test]
fn a_shared_target_dir_never_gives_one_entry_the_executable_of_another() {
    let dir = fresh("target-dir");
    let first = dir.join("first.ignite");
    let second = dir.join("second.ignite");
    std::fs::write(&first, program("first")).unwrap();
    std::fs::write(&second, program("second")).unwrap();
    let target_dir = shared_cargo_target_dir();
    let first_build = tmp("target-dir-first-build");
    let second_build = tmp("target-dir-second-build");
    for build in [&first_build, &second_build] {
        let _ = std::fs::remove_dir_all(build);
    }
    let printed = [
        run(
            &first,
            &first_build,
            "rust",
            Some(&target_dir),
            "target-dir-1",
        ),
        run(
            &second,
            &second_build,
            "rust",
            Some(&target_dir),
            "target-dir-2",
        ),
        run(
            &first,
            &first_build,
            "rust",
            Some(&target_dir),
            "target-dir-1-again",
        ),
    ];
    assert_eq!(
        printed,
        ["first\n", "second\n", "first\n"],
        "the third run is the first entry again"
    );
}

/// The stamp holds the entry's canonical path, so the same entry reached
/// through a symlink beside it reuses the build: the stamp does not change
/// and the run prints the same. Hashing the path as given would rebuild.
#[test]
fn the_same_entry_through_a_symlink_reuses_the_build() {
    let dir = fresh("symlink");
    let build = tmp("symlink-build");
    let _ = std::fs::remove_dir_all(&build);
    let entry = dir.join("first.ignite");
    std::fs::write(&entry, program("first")).unwrap();
    let via = dir.join("via.ignite");
    std::os::unix::fs::symlink("first.ignite", &via).unwrap();
    assert_eq!(run(&entry, &build, "llvm", None, "symlink-1"), "first\n");
    let built = stamp(&build);
    assert_eq!(run(&via, &build, "llvm", None, "symlink-2"), "first\n");
    assert_eq!(
        stamp(&build),
        built,
        "the same entry through a symlink rebuilt"
    );
}
