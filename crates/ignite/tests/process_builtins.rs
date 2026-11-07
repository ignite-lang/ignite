//! `docs/slice.md` 21.20 end to end: the pusher case study's reproducers
//! G2, G3, G8 and G13 (`docs/research/pusher-gaps.md`), adapted to the
//! names chosen there, in `fixtures/process_builtins/pusher_gaps.ignite`:
//! built once per target and started as real processes, its inline test
//! run on both targets.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use ignite::Target;

use crate::common;

const SOURCE: &str = "pusher_gaps.ignite";

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/process_builtins")
        .join(name)
}

fn tmp(name: impl AsRef<Path>) -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("process_builtins")
        .join(name)
}

fn target_name(target: Target) -> &'static str {
    match target {
        Target::Rust => "rust",
        Target::Llvm => "llvm",
    }
}

/// The executable `name` of the fixture, built once per target.
fn executable(name: &str, target: Target) -> PathBuf {
    static BUILT: Mutex<Vec<(&'static str, Vec<PathBuf>)>> = Mutex::new(Vec::new());
    let key = target_name(target);
    let mut built = BUILT.lock().unwrap_or_else(|e| e.into_inner());
    if !built.iter().any(|(t, _)| *t == key) {
        let out = tmp(format!("build-{key}"));
        let options = ignite::BuildOptions {
            target,
            out: Some(out),
            runtime: (target == Target::Llvm).then(common::runtime_archive),
            ..Default::default()
        };
        let output = ignite::build(&fixture(SOURCE), &options).unwrap_or_else(|e| panic!("{e}"));
        built.push((key, output.executables));
    }
    let (_, executables) = built
        .iter()
        .find(|(t, _)| *t == key)
        .expect("built just now");
    executables
        .iter()
        .find(|path| path.file_stem().and_then(|s| s.to_str()) == Some(name))
        .unwrap_or_else(|| panic!("no {name} in {executables:?}"))
        .clone()
}

/// `name` run to its end with `PUSHER_SESSION_STORE` set to `store`, or
/// unset: its output.
fn run(name: &str, target: Target, store: Option<&str>) -> Output {
    let path = executable(name, target);
    let mut command = Command::new(&path);
    command
        .stdin(Stdio::null())
        .env_remove("PUSHER_SESSION_STORE");
    if let Some(store) = store {
        command.env("PUSHER_SESSION_STORE", store);
    }
    command
        .output()
        .unwrap_or_else(|e| panic!("run {}: {e}", path.display()))
}

fn stderr(output: &Output) -> String {
    let text = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(output.status.code(), Some(0), "{text}");
    text
}

macro_rules! both_targets {
    ($($case:ident),* $(,)?) => {
        mod rust {
            $(#[test]
            fn $case() {
                super::$case(ignite::Target::Rust);
            })*
        }
        mod llvm {
            $(#[test]
            fn $case() {
                super::$case(ignite::Target::Llvm);
            })*
        }
    };
}

both_targets!(
    g2_reads_an_environment_variable,
    g3_sleeps,
    g8_reads_the_wall_clock,
);

/// G2: the `Environment` resource holds the process environment at launch.
fn g2_reads_an_environment_variable(target: Target) {
    let set = run("g2", target, Some("redis://sessions:6379"));
    assert_eq!(stderr(&set), "Some(\"redis://sessions:6379\")\n");
    let unset = run("g2", target, None);
    assert_eq!(stderr(&unset), "None\n");
}

/// G3: `Clock.sleep(300 ms)` waits at least 300 ms of the monotonic clock.
fn g3_sleeps(target: Target) {
    executable("g3", target);
    let started = Instant::now();
    let output = run("g3", target, None);
    let took = started.elapsed();
    let text = stderr(&output);
    let slept: u64 = text
        .trim()
        .strip_prefix("slept ")
        .and_then(|ms| ms.parse().ok())
        .unwrap_or_else(|| panic!("{text}"));
    assert!((300..5000).contains(&slept), "{text}");
    assert!(took.as_millis() >= 300, "{took:?}");
}

/// G8: `Clock.now().to_ms()` is the wall clock in milliseconds since the
/// Unix epoch.
fn g8_reads_the_wall_clock(target: Target) {
    executable("g8", target);
    let before = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let output = run("g8", target, None);
    let after = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let text = stderr(&output);
    let ms: u128 = text.trim().parse().unwrap_or_else(|_| panic!("{text}"));
    assert!(
        before.as_millis() <= ms && ms <= after.as_millis(),
        "{} <= {ms} <= {}",
        before.as_millis(),
        after.as_millis()
    );
}

/// G13: `Process.flush()` is callable; the fixture's inline test runs on
/// both targets.
#[test]
fn g13_flush_is_callable_on_both_targets() {
    let dir = tmp("inline");
    let output = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("test")
        .arg(fixture(SOURCE))
        .args(["--target", "all", "--out"])
        .arg(&dir)
        .arg("--target-dir")
        .arg(dir.join("cargo"))
        .arg("--runtime")
        .arg(common::runner_archive())
        .output()
        .expect("run ignite test");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(
        stdout
            .matches("test result: ok. 1 passed; 0 failed;")
            .count(),
        2,
        "{stdout}"
    );
}
