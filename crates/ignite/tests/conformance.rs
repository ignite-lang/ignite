//! `ignite conformance` end to end (`docs/slice.md` 19.10, rows 31 to 37 of
//! 19.0): the fixtures under `tests/fixtures/conformance/` each declare their
//! own small catalogue and one protocol that conforms to it; the matrix on
//! stdout is compared with the fixture's `.expected`, and the exit status
//! with its claim lines.
//!
//! The Rust test builds share one cargo target directory, so the generated
//! `ignite-std` and the substrate are compiled once; [`BUILDS`] serialises
//! them, because each build claims the directory (`claim_target_dir`).

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::common;

static BUILDS: Mutex<()> = Mutex::new(());

/// The fixtures of row 31, one per verdict.
const VERDICTS: [&str; 8] = [
    "pass",
    "fail",
    "gap",
    "deviation",
    "inconclusive",
    "not_applicable",
    "out_of_scope",
    "unclassified",
];

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/conformance")
        .join(format!("{name}.ignite"))
}

fn expected(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/conformance")
        .join(format!("{name}.expected"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn tmp(name: &str) -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("conformance")
        .join(name)
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// `ignite conformance <fixture> --target <target>` with its builds under
/// `tmp(out)`, the Rust builds in the shared cargo target directory.
fn command(name: &str, target: &str, out: &str, extra: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ignite"));
    command
        .arg("conformance")
        .arg(fixture(name))
        .args(["--target", target, "--out"])
        .arg(tmp(out))
        .args(extra);
    if target == "rust" {
        command.arg("--target-dir").arg(tmp("cargo"));
    } else {
        command.arg("--runtime").arg(common::runner_archive());
    }
    command
}

fn conformance(name: &str, target: &str) -> Output {
    let _guard = BUILDS.lock().unwrap_or_else(|e| e.into_inner());
    command(name, target, &format!("{name}-{target}"), &[])
        .output()
        .expect("run ignite conformance")
}

/// The exit status the claim lines of `stdout` call for (19.10).
fn claim_status(stdout: &str) -> i32 {
    let claims: Vec<&str> = stdout
        .lines()
        .filter(|line| line.starts_with("claim conformance of "))
        .collect();
    assert!(!claims.is_empty(), "no claim line in {stdout}");
    i32::from(claims.iter().any(|line| line.contains(": no (")))
}

fn assert_matrix(name: &str, output: &Output) {
    let stdout = text(&output.stdout);
    let want = expected(name);
    assert_eq!(stdout, want, "{name}: stderr: {}", text(&output.stderr));
    assert_eq!(
        output.status.code(),
        Some(claim_status(&want)),
        "{name}: stderr: {}",
        text(&output.stderr)
    );
}

#[test]
fn each_verdict_prints_its_row() {
    for name in VERDICTS {
        let output = conformance(name, "rust");
        assert_matrix(name, &output);
    }
}

#[test]
fn a_failing_verifies_test_fails_the_row_and_the_claim() {
    let output = conformance("fail", "rust");
    let stdout = text(&output.stdout);
    let row = stdout
        .lines()
        .find(|line| line.starts_with("R2_1 "))
        .unwrap_or_else(|| panic!("no row in {stdout}"));
    assert!(row.ends_with("  fail"), "{row}");
    assert!(
        stdout.contains(
            "claim conformance of Echo to rfc7102: no (0 gaps, 1 failures, 0 inconclusive)\n"
        ),
        "{stdout}"
    );
    assert_eq!(output.status.code(), Some(1), "{}", text(&output.stderr));
}

#[test]
fn satisfied_without_a_passing_test_is_inconclusive() {
    let output = conformance("inconclusive", "rust");
    let stdout = text(&output.stdout);
    let row = stdout
        .lines()
        .find(|line| line.starts_with("R2_1 "))
        .unwrap_or_else(|| panic!("no row in {stdout}"));
    assert!(row.contains("  echo; no test  "), "{row}");
    assert!(row.ends_with("  inconclusive"), "{row}");
    assert_matrix("inconclusive", &output);
}

#[test]
fn a_complete_claim_holds() {
    let output = conformance("complete", "rust");
    let stdout = text(&output.stdout);
    for line in stdout.lines().filter(|line| line.starts_with('R')) {
        if line.contains("  MUST") {
            assert!(line.ends_with("  pass"), "{line}");
        }
    }
    assert!(
        stdout.contains(
            "claim conformance of Echo to rfc7110: yes (0 gaps, 0 failures, 0 inconclusive)\n"
        ),
        "{stdout}"
    );
    assert_matrix("complete", &output);
    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn an_inconclusive_must_breaks_the_claim() {
    let output = conformance("inconclusive_must", "rust");
    let stdout = text(&output.stdout);
    let rows: Vec<&str> = stdout
        .lines()
        .filter(|line| line.starts_with('R'))
        .collect();
    let others: Vec<&&str> = rows.iter().filter(|row| !row.ends_with("  pass")).collect();
    assert_eq!(others.len(), 1, "{stdout}");
    assert!(others[0].ends_with("  inconclusive"), "{stdout}");
    assert!(
        stdout.contains(
            "claim conformance of Echo to rfc7111: no (0 gaps, 0 failures, 1 inconclusive)\n"
        ),
        "{stdout}"
    );
    assert_matrix("inconclusive_must", &output);
    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn output_is_deterministic() {
    let first = conformance("complete", "rust");
    let second = conformance("complete", "rust");
    assert_eq!(first.status.code(), Some(0), "{}", text(&first.stderr));
    assert_eq!(text(&first.stdout), text(&second.stdout));
    assert_eq!(first.status.code(), second.status.code());
}

#[test]
fn both_targets_print_the_same_matrix() {
    let rust = conformance("complete", "rust");
    let llvm = conformance("complete", "llvm");
    assert_matrix("complete", &rust);
    assert_matrix("complete", &llvm);
    assert_eq!(text(&rust.stdout), text(&llvm.stdout));
}

/// Every process whose executable is `exe`.
fn processes_of(exe: &Path) -> Vec<(i32, Vec<String>)> {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let pid: i32 = entry.file_name().to_str()?.parse().ok()?;
            let link = std::fs::read_link(entry.path().join("exe")).ok()?;
            if link != exe {
                return None;
            }
            let cmdline = std::fs::read(entry.path().join("cmdline")).ok()?;
            let args = cmdline
                .split(|byte| *byte == 0)
                .filter(|arg| !arg.is_empty())
                .map(|arg| String::from_utf8_lossy(arg).into_owned())
                .collect();
            Some((pid, args))
        })
        .collect()
}

fn kill_all(exe: &Path) {
    for (pid, _) in processes_of(exe) {
        let _ = nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(pid),
            nix::sys::signal::Signal::SIGKILL,
        );
    }
}

/// Kills every process of the test executable when dropped, so that a
/// failed assertion leaves no test child spinning.
struct Reaper(PathBuf);

impl Drop for Reaper {
    fn drop(&mut self) {
        kill_all(&self.0);
    }
}

/// `signal.ignite`'s only test never ends; once the runner has started it,
/// the runner itself is killed with SIGKILL.
fn killed_runner_exit() -> Output {
    let _guard = BUILDS.lock().unwrap_or_else(|e| e.into_inner());
    let out = tmp("signal-rust");
    let exe = out.join("tests/rust/ignite-test");
    let mut child = command("signal", "rust", "signal-rust", &[])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn ignite conformance");
    let reaper = Reaper(exe.clone());
    let deadline = Instant::now() + Duration::from_secs(300);
    loop {
        let exe = exe.canonicalize().unwrap_or_else(|_| exe.clone());
        let processes = processes_of(&exe);
        let started = processes
            .iter()
            .any(|(_, args)| args.iter().any(|arg| arg == "--run-one"));
        let runner = processes
            .iter()
            .find(|(_, args)| !args.iter().any(|arg| arg == "--run-one"));
        if let (true, Some((pid, _))) = (started, runner) {
            nix::sys::signal::kill(
                nix::unistd::Pid::from_raw(*pid),
                nix::sys::signal::Signal::SIGKILL,
            )
            .expect("kill the runner");
            kill_all(&exe);
            break;
        }
        if let Some(status) = child.try_wait().expect("poll ignite conformance") {
            panic!("ignite conformance ended before its test ran: {status}");
        }
        assert!(
            Instant::now() < deadline,
            "the runner never started its test"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let output = child
        .wait_with_output()
        .expect("wait for ignite conformance");
    drop(reaper);
    output
}

#[test]
fn conformance_exit_statuses() {
    let diagnostic = conformance("diagnostic", "rust");
    assert_eq!(
        diagnostic.status.code(),
        Some(2),
        "{}",
        text(&diagnostic.stderr)
    );
    assert_eq!(text(&diagnostic.stdout), "");
    let stderr = text(&diagnostic.stderr);
    assert!(
        stderr.contains("diagnostic.ignite:") && stderr.contains("error[E090]"),
        "{stderr}"
    );

    let no_claim = conformance("no_claim", "rust");
    assert_eq!(
        no_claim.status.code(),
        Some(2),
        "{}",
        text(&no_claim.stderr)
    );
    assert_eq!(text(&no_claim.stdout), "");
    assert_eq!(
        text(&no_claim.stderr).lines().count(),
        1,
        "{}",
        text(&no_claim.stderr)
    );

    let build = command("pass", "llvm", "build-fails", &["--clang", "/bin/false"])
        .output()
        .expect("run ignite conformance");
    assert_eq!(build.status.code(), Some(3), "{}", text(&build.stderr));
    assert_eq!(text(&build.stdout), "");
    assert!(
        text(&build.stderr).contains("error: clang failed"),
        "{}",
        text(&build.stderr)
    );

    let killed = killed_runner_exit();
    assert_eq!(killed.status.code(), Some(3), "{}", text(&killed.stderr));
    assert_eq!(text(&killed.stdout), "");
    assert!(
        text(&killed.stderr).contains("the test executable was killed by signal 9"),
        "{}",
        text(&killed.stderr)
    );
}
