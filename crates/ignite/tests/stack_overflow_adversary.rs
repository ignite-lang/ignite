//! Adversary pass 2 on `story:http-reply-cost-back` (fix round 5): on the
//! LLVM target the runtime gives every worker an alternate signal stack,
//! not only the first. Sixteen tasks overflow at staggered times on four
//! workers, on a default start and on one with SIGSEGV and SIGBUS
//! ignored, three rounds each: every round reports and aborts.

use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

const LIMIT: Duration = Duration::from_secs(20);

const PROGRAM: &str = r#"package tests.stack_overflow_adversary

fn deeper(depth: Int) -> Int = deeper(depth + 1) + depth

fn dive(wait: Int) -> Result<Unit, IoError> {
  Clock.sleep(wait * 3 ms)
  log("reached {deeper(0)}")
  Ok(())
}

fn hold() -> Result<Unit, IoError> {
  Clock.sleep(30000 ms)
  Ok(())
}

type Divers {
  executable: Text
}

fn Divers.main(self: Divers) -> Result<Unit, IoError> {
  spawn hold()
  spawn hold()
  spawn hold()
  spawn dive(16)
  spawn dive(15)
  spawn dive(14)
  spawn dive(13)
  spawn dive(12)
  spawn dive(11)
  spawn dive(10)
  spawn dive(9)
  spawn dive(8)
  spawn dive(7)
  spawn dive(6)
  spawn dive(5)
  spawn dive(4)
  spawn dive(3)
  spawn dive(2)
  spawn dive(1)
  Clock.sleep(30000 ms)
  log("the overflow did not end the process")
  Process.exit(9)
}

system StackOverflowAdversary {
  component divers: Divers { executable = "divers" }
}
"#;

const REPORT: &str = "thread 'worker' has overflowed its stack";

fn divers() -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("stack-overflow-adversary-llvm");
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("stack_overflow_adversary.ignite");
    std::fs::write(&source, PROGRAM).unwrap();
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("build")
        .arg(&source)
        .args(["--target", "llvm", "--out"])
        .arg(dir.join("out"))
        .arg("--runtime")
        .arg(crate::common::runtime_archive())
        .current_dir(&repo)
        .output()
        .expect("run ignite build");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        output.status.success(),
        "ignite build failed\n{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    stdout
        .lines()
        .filter_map(|line| line.strip_prefix("built "))
        .find(|path| path.ends_with("/divers"))
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("no divers in\n{stdout}"))
}

fn run(binary: &Path, trap: &str) -> (Output, bool) {
    let mut child = Command::new("sh")
        .args(["-c", &format!("{trap}exec \"$0\"")])
        .arg(binary)
        .env("IGNITE_WORKERS", "4")
        .env_remove("IGNITE_IO")
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

#[test]
fn llvm_overflows_on_four_workers_are_reported_on_every_start() {
    let binary = divers();
    for trap in ["", "trap '' SEGV BUS; ", "trap '' SEGV; ", "trap '' BUS; "] {
        for round in 0..3 {
            let (output, hung) = run(&binary, trap);
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(!hung, "{trap:?} round {round}: hung\n{stderr}");
            assert!(
                stderr.contains(REPORT),
                "{trap:?} round {round}: no report, {:?}\n{stderr}",
                output.status
            );
            assert_eq!(
                output.status.signal(),
                Some(6),
                "{trap:?} round {round}: {:?}\n{stderr}",
                output.status
            );
        }
    }
}
