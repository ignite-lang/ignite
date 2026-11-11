//! A task's stack overflow on both targets (story `http-reply-cost-back`,
//! fix round 5, `docs/slice.md` section NN): a default start reports it
//! and aborts; a start with SIGSEGV and SIGBUS ignored reports it on the
//! LLVM target, whose runtime gives every worker an alternate signal stack,
//! and on the Rust target ends by the fault's signal, as at v0.15.0, since
//! std gives its threads no alternate stack then.
//!
//! The program is written to this test's scratch directory, not kept in
//! `fixtures/`: the differential runner would run its main on every target.

use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// The longest a program may take to end after its start.
const LIMIT: Duration = Duration::from_secs(20);

const PROGRAM: &str = r#"package tests.stack_overflow

fn deeper(depth: Int) -> Int = deeper(depth + 1) + depth

fn dive() -> Result<Unit, IoError> {
  Clock.sleep(1 ms)
  log("diving")
  log("reached {deeper(0)}")
  Ok(())
}

type Diver {
  executable: Text
}

fn Diver.main(self: Diver) -> Result<Unit, IoError> {
  spawn dive()
  Clock.sleep(30000 ms)
  log("the overflow did not end the process")
  Process.exit(9)
}

system StackOverflow {
  component diver: Diver { executable = "diver" }
}
"#;

const REPORT: &str = "thread 'worker' has overflowed its stack";

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// `diver` built for `target` (dev), the LLVM target with the runtime
/// archive of this test build.
fn diver(target: &str) -> PathBuf {
    let dir = scratch(&format!("stack-overflow-{target}"));
    let source = dir.join("stack_overflow.ignite");
    std::fs::write(&source, PROGRAM).unwrap();
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut command = Command::new(env!("CARGO_BIN_EXE_ignite"));
    command
        .arg("build")
        .arg(&source)
        .args(["--target", target, "--out"])
        .arg(dir.join("out"))
        .current_dir(&repo);
    if target == "llvm" {
        command
            .arg("--runtime")
            .arg(crate::common::runtime_archive());
    } else {
        command
            .arg("--target-dir")
            .arg(Path::new(env!("CARGO_TARGET_TMPDIR")).join("run-cargo"));
    }
    let output = command.output().expect("run ignite build");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        output.status.success(),
        "{target}: ignite build failed\n{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    stdout
        .lines()
        .filter_map(|line| line.strip_prefix("built "))
        .find(|path| path.ends_with("/diver"))
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("{target}: no diver in\n{stdout}"))
}

/// Run `binary`, through a shell that ignores SIGSEGV and SIGBUS first
/// when `ignored` (`trap ''`, which `exec` keeps); its output and whether
/// it had to be killed after [`LIMIT`].
fn run(binary: &Path, ignored: bool) -> (Output, bool) {
    let trap = if ignored { "trap '' SEGV BUS; " } else { "" };
    let mut child = Command::new("sh")
        .args(["-c", &format!("{trap}exec \"$0\"")])
        .arg(binary)
        .env("IGNITE_WORKERS", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let start = Instant::now();
    let mut hung = false;
    while child.try_wait().unwrap().is_none() {
        if start.elapsed() > LIMIT {
            hung = true;
            let _ = child.kill();
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    (child.wait_with_output().unwrap(), hung)
}

/// The report and the abort (SIGABRT) of a task's overflow.
fn reported(target: &str, start: &str, output: &Output, hung: bool) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !hung,
        "{target}, {start}: not ended within {LIMIT:?}\n{stderr}"
    );
    assert!(
        stderr.contains(REPORT),
        "{target}, {start}: no report\n{stderr}"
    );
    assert_eq!(
        output.status.signal(),
        Some(6),
        "{target}, {start}: {:?}\n{stderr}",
        output.status
    );
}

/// A default start: the report and the abort on both targets.
#[test]
fn a_task_overflow_is_reported_on_both_targets() {
    std::thread::scope(|scope| {
        for target in ["rust", "llvm"] {
            scope.spawn(move || {
                let (output, hung) = run(&diver(target), false);
                reported(target, "default start", &output, hung);
            });
        }
    });
}

/// Started with SIGSEGV and SIGBUS ignored: the LLVM target still reports
/// (its runtime gives each worker an alternate stack); the Rust target
/// ends by SIGSEGV or SIGBUS with no report, within the wait.
#[test]
fn started_with_faults_ignored_llvm_reports_and_rust_ends_by_the_signal() {
    std::thread::scope(|scope| {
        scope.spawn(|| {
            let (output, hung) = run(&diver("llvm"), true);
            reported("llvm", "faults ignored", &output, hung);
        });
        scope.spawn(|| {
            let (output, hung) = run(&diver("rust"), true);
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(!hung, "rust: not ended within {LIMIT:?}\n{stderr}");
            assert!(
                matches!(output.status.signal(), Some(11 | 7)),
                "rust: ended by {:?}, not SIGSEGV or SIGBUS\n{stderr}",
                output.status
            );
            assert!(
                !stderr.contains(REPORT),
                "rust: a report without an alternate stack\n{stderr}"
            );
        });
    });
}
