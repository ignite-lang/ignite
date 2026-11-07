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

// Adversary cases (slice 15, U5). Each expected matrix below is written from
// 19.10 alone: `adversary_block` pads the rows by the contract's rule, not by
// the implementation's.

/// One block of 19.10: the header, the rows with their six columns joined by
/// two spaces and padded (in characters) to the widest entry of their column,
/// the last column unpadded, and the claim line.
fn adversary_block(header: &str, rows: &[[&str; 6]], claim: &str) -> String {
    let mut widths = [0usize; 6];
    for row in rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.chars().count());
        }
    }
    let mut out = format!("{header}\n");
    for row in rows {
        for (column, cell) in row.iter().enumerate() {
            out.push_str(cell);
            if column < 5 {
                out.push_str(&" ".repeat(widths[column] - cell.chars().count() + 2));
            }
        }
        out.push('\n');
    }
    out.push_str(claim);
    out.push('\n');
    out
}

/// `ignite conformance FILE --target TARGET` on any path, builds under
/// `tmp(out)`, run in `dir` when given.
fn adversary_command(file: &Path, target: &str, out: &str, dir: Option<&Path>) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ignite"));
    command
        .arg("conformance")
        .arg(file)
        .args(["--target", target, "--out"])
        .arg(tmp(out));
    if target == "rust" {
        command.arg("--target-dir").arg(tmp("cargo"));
    } else {
        command.arg("--runtime").arg(common::runner_archive());
    }
    if let Some(dir) = dir {
        command.current_dir(dir);
    }
    command
}

/// 19.10: "The claim holds (`yes`) when no MUST-class row in scope is `fail`,
/// `gap` or `inconclusive`. The counts are over all rows in scope, at any
/// level". A SHOULD that fails and a SHOULD without a test are counted and
/// keep the claim; an out-of-scope MUST whose test exits 7 is counted nowhere;
/// a test that panics and one that exits with 7 are both `failed`.
#[test]
fn adversary_should_failures_are_counted_and_keep_the_claim() {
    let output = conformance("adversary_should_failure", "rust");
    let want = adversary_block(
        "rfc7301 Line Echo, protocol Echo, parties Server, sections all",
        &[
            [
                "R2_1",
                "2",
                "MUST",
                "satisfied",
                "echo; test \"a line is echoed\" passed",
                "pass",
            ],
            [
                "R2_2",
                "2",
                "SHOULD",
                "satisfied",
                "echo; test \"a line is echoed\" passed, test \"an index out of range fails\" failed",
                "fail",
            ],
            [
                "R2_3",
                "2",
                "SHOULD",
                "satisfied",
                "keep; no test",
                "inconclusive",
            ],
            [
                "R2_4",
                "2",
                "MAY",
                "unsupported",
                "\"nothing is logged\"",
                "n/a",
            ],
            [
                "R2_5",
                "2",
                "SHOULD",
                "unsupported",
                "\"no answer is timed\"",
                "deviation",
            ],
            [
                "R4_1",
                "4",
                "MUST",
                "satisfied",
                "forward; test \"a forwarded line exits with seven\" failed",
                "out of scope",
            ],
        ],
        "claim conformance of Echo to rfc7301: yes (0 gaps, 1 failures, 1 inconclusive)",
    );
    assert_eq!(text(&output.stdout), want, "{}", text(&output.stderr));
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
}

/// The three blocks of `adversary_two_specs/main.ignite`, in protocol then
/// `conforms` order (19.10 step 4), one blank line between blocks.
fn adversary_two_specs_matrix() -> String {
    let echoed = "echo; test \"a line is echoed\" passed";
    let padded =
        "pad; test \"a short line is padded\" passed, test \"padding exits with seven\" failed";
    [
        adversary_block(
            "rfc7202 Line Echo Two, protocol Zeta, parties all, sections 1",
            &[
                ["R1_1", "1", "MUST", "satisfied", echoed, "pass"],
                ["R1_2", "1", "SHOULD", "none", "-", "unclassified"],
            ],
            "claim conformance of Zeta to rfc7202: yes (0 gaps, 0 failures, 0 inconclusive)",
        ),
        adversary_block(
            "rfc7201 Line Echo, protocol Zeta, parties all, sections 1",
            &[
                ["R1_1", "1", "MUST", "satisfied", echoed, "pass"],
                ["R2_1", "2", "MUST", "none", "-", "out of scope"],
                ["R3_1", "3", "MUST", "satisfied", padded, "out of scope"],
            ],
            "claim conformance of Zeta to rfc7201: yes (0 gaps, 0 failures, 0 inconclusive)",
        ),
        adversary_block(
            "rfc7201 Line Echo, protocol Alpha, parties all, sections all",
            &[
                ["R1_1", "1", "MUST", "satisfied", echoed, "pass"],
                [
                    "R2_1",
                    "2",
                    "MUST",
                    "unsupported",
                    "\"sessions never idle out\"",
                    "gap",
                ],
                ["R3_1", "3", "MUST", "satisfied", padded, "fail"],
            ],
            "claim conformance of Alpha to rfc7201: no (1 gaps, 1 failures, 0 inconclusive)",
        ),
    ]
    .join("\n")
}

/// Two protocols, one of them conforming to a catalogue of a sibling module
/// and to the file's own: one block and one claim line per (protocol, spec),
/// declaration then `conforms` order, exit 1 because one claim fails, and
/// byte-identical stdout on both targets (19.10 steps 3 and 4, row 36).
#[test]
fn adversary_two_protocols_and_two_specs_print_in_order_on_both_targets() {
    let want = adversary_two_specs_matrix();
    for target in ["rust", "llvm"] {
        let output = conformance("adversary_two_specs/main", target);
        assert_eq!(
            text(&output.stdout),
            want,
            "{target}: {}",
            text(&output.stderr)
        );
        assert_eq!(
            output.status.code(),
            Some(1),
            "{target}: {}",
            text(&output.stderr)
        );
    }
}

/// 19.10: a test build failure is exit 3 and nothing on stdout, also when a
/// claim of the file fails; a file whose rows name no test builds nothing, so
/// a broken clang does not matter to it.
#[test]
fn adversary_a_build_failure_outranks_a_failing_claim() {
    let build = command(
        "adversary_two_specs/main",
        "llvm",
        "adversary-build-fails",
        &["--clang", "/bin/false"],
    )
    .output()
    .expect("run ignite conformance");
    assert_eq!(build.status.code(), Some(3), "{}", text(&build.stderr));
    assert_eq!(text(&build.stdout), "");

    let unbuilt = command(
        "gap",
        "llvm",
        "adversary-no-build",
        &["--clang", "/bin/false"],
    )
    .output()
    .expect("run ignite conformance");
    assert_eq!(
        text(&unbuilt.stdout),
        expected("gap"),
        "{}",
        text(&unbuilt.stderr)
    );
    assert_eq!(unbuilt.status.code(), Some(1), "{}", text(&unbuilt.stderr));
}

/// Unicode and quotes in a title, a test name and a reason, and the file
/// given by two different paths (absolute, and relative from a directory
/// whose name holds a space and an umlaut): the same matrix on stdout, no path
/// in it, and the same on LLVM (rows 36, 19.10).
#[test]
fn adversary_unicode_names_and_paths_do_not_change_the_matrix() {
    let want = adversary_block(
        "rfc7303 Zeilen-Echo – „Größe“, protocol Echo, parties all, sections all",
        &[
            [
                "R2_1",
                "2",
                "MUST",
                "satisfied",
                "echo; test \"Größe \"ü\" ✓ wird geechot\" passed",
                "pass",
            ],
            [
                "R2_2",
                "2",
                "SHOULD",
                "unsupported",
                "\"kein \"Timeout\" – nie ✓\"",
                "deviation",
            ],
        ],
        "claim conformance of Echo to rfc7303: yes (0 gaps, 0 failures, 0 inconclusive)",
    );
    let source = fixture("adversary_unicode");
    let plain = tmp("adversary-paths/plain");
    let spaced = tmp("adversary-paths/mit Leerzeichen ü");
    for dir in [&plain, &spaced] {
        std::fs::create_dir_all(dir).expect("create the copy's directory");
        std::fs::copy(&source, dir.join("copy.ignite")).expect("copy the fixture");
    }
    let _guard = BUILDS.lock().unwrap_or_else(|e| e.into_inner());
    let runs = [
        adversary_command(&plain.join("copy.ignite"), "rust", "adversary-plain", None),
        adversary_command(
            Path::new("copy.ignite"),
            "rust",
            "adversary-spaced",
            Some(&spaced),
        ),
        adversary_command(&source, "llvm", "adversary-unicode-llvm", None),
    ];
    for mut run in runs {
        let output = run.output().expect("run ignite conformance");
        let stdout = text(&output.stdout);
        assert_eq!(stdout, want, "{}", text(&output.stderr));
        assert!(
            !stdout.contains("copy.ignite") && !stdout.contains(env!("CARGO_TARGET_TMPDIR")),
            "{stdout}"
        );
        assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    }
}

/// 19.5's grammar admits `parties = []` and `sections = []`; 19.10: "no line
/// ends in a space". The header of such a block must not end in one.
#[test]
fn adversary_an_empty_scope_list_leaves_no_trailing_space() {
    let output = conformance("adversary_empty_scope", "rust");
    let stdout = text(&output.stdout);
    assert!(
        stdout.starts_with("rfc7304 Line Echo, protocol Echo, parties none, sections none\n"),
        "{stdout}"
    );
    assert!(
        stdout.contains("R2_1  2  MUST  none  -  out of scope\n"),
        "{stdout}{}",
        text(&output.stderr)
    );
    for line in stdout.lines() {
        assert!(!line.ends_with(' '), "line ends in a space: {line:?}");
    }
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
}

/// 19.7 and 19.10 limits: a `ByteSize` default in bytes, an Int below its
/// minimum (`fail`, at MUST and at SHOULD), a Text field and a field without
/// a default (no value, `inconclusive`); no test is named, so nothing is
/// built.
#[test]
fn adversary_limits_print_their_values_and_are_counted() {
    let output = conformance("adversary_limits", "rust");
    let want = adversary_block(
        "rfc7306 Line Echo, protocol Echo, parties all, sections all",
        &[
            [
                "R2_1",
                "2",
                "MUST",
                "limit",
                "EchoLimits.head = 8192, minimum 8000",
                "pass",
            ],
            [
                "R2_2",
                "2",
                "MUST",
                "limit",
                "EchoLimits.lines = 63, minimum 64",
                "fail",
            ],
            [
                "R2_3",
                "2",
                "MUST",
                "limit",
                "EchoLimits.name has no value, minimum 1",
                "inconclusive",
            ],
            [
                "R2_4",
                "2",
                "MUST",
                "limit",
                "EchoLimits.sessions has no value, minimum 1",
                "inconclusive",
            ],
            [
                "R2_5",
                "2",
                "SHOULD",
                "limit",
                "EchoLimits.history = 5, minimum 100",
                "fail",
            ],
        ],
        "claim conformance of Echo to rfc7306: no (0 gaps, 2 failures, 2 inconclusive)",
    );
    assert_eq!(text(&output.stdout), want, "{}", text(&output.stderr));
    assert_eq!(output.status.code(), Some(1), "{}", text(&output.stderr));
}

/// 19.10 step 3: a test name holding a blank line (`\n\n`, a legal escape)
/// does not end the result lines, so the verifying test declared after it
/// still reads `passed` and the row `pass`, not `not run` and `inconclusive`.
#[test]
fn a_blank_line_in_a_test_name_does_not_end_the_results() {
    for target in ["rust", "llvm"] {
        let output = conformance("blank_line_name", target);
        assert_eq!(
            text(&output.stdout),
            expected("blank_line_name"),
            "{target}: {}",
            text(&output.stderr)
        );
        assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    }
}

/// 19.10 verdict rule 5: "`fail` when any verifying test failed". The only
/// verifying test fails; another test's name (`\n` is a legal escape, 12.1)
/// makes the runner print a second `test <module>/a line is echoed ... ok`
/// line after the real `FAILED` one. The row must stay `fail` and the claim
/// `no`.
#[test]
fn adversary_a_test_name_cannot_forge_another_tests_result() {
    let output = conformance("adversary_forged_result", "rust");
    let want = adversary_block(
        "rfc7305 Line Echo, protocol Echo, parties all, sections all",
        &[[
            "R2_1",
            "2",
            "MUST",
            "satisfied",
            "echo; test \"a line is echoed\" failed",
            "fail",
        ]],
        "claim conformance of Echo to rfc7305: no (0 gaps, 1 failures, 0 inconclusive)",
    );
    assert_eq!(text(&output.stdout), want, "{}", text(&output.stderr));
    assert_eq!(output.status.code(), Some(1), "{}", text(&output.stderr));
}

/// 19.10 step 1: a diagnostic is printed "as `ignite check` does" (the same
/// stderr), exit 2, no stdout; a missing file is an IO error, exit 3; a usage
/// error is exit 2. None prints on stdout.
#[test]
fn adversary_exit_statuses_follow_d8() {
    let check = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("check")
        .arg(fixture("diagnostic"))
        .output()
        .expect("run ignite check");
    assert_eq!(check.status.code(), Some(1));
    let diagnostic = conformance("diagnostic", "rust");
    assert_eq!(diagnostic.status.code(), Some(2));
    assert_eq!(text(&diagnostic.stdout), "");
    assert_eq!(text(&diagnostic.stderr), text(&check.stderr));

    let missing = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("conformance")
        .arg(tmp("adversary-missing/none.ignite"))
        .output()
        .expect("run ignite conformance");
    assert_eq!(missing.status.code(), Some(3), "{}", text(&missing.stderr));
    assert_eq!(text(&missing.stdout), "");

    let usage = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("conformance")
        .arg(fixture("pass"))
        .args(["--target", "wasm"])
        .output()
        .expect("run ignite conformance");
    assert_eq!(usage.status.code(), Some(2), "{}", text(&usage.stderr));
    assert_eq!(text(&usage.stdout), "");
}

// The claims of the standard library (slice 15, U6; rows 1 to 5 of 19.0).

/// The repository root, two levels above this crate.
fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// `ignite conformance std/protocol/<name>.ignite --target <target>`.
fn std_conformance(name: &str, target: &str) -> Output {
    let file = repository().join(format!("std/protocol/{name}.ignite"));
    let _guard = BUILDS.lock().unwrap_or_else(|e| e.into_inner());
    adversary_command(&file, target, &format!("std-{name}-{target}"), None)
        .output()
        .expect("run ignite conformance")
}

/// The block of `stdout` whose header starts with `rfc<n> `.
fn block<'a>(stdout: &'a str, rfc: &str) -> &'a str {
    stdout
        .split("\n\n")
        .find(|block| block.starts_with(&format!("{rfc} ")))
        .unwrap_or_else(|| panic!("no {rfc} block in {stdout}"))
}

/// The row of `id` in `block`, split into its columns at runs of two spaces.
fn columns<'a>(block: &'a str, id: &str) -> Vec<&'a str> {
    let row = block
        .lines()
        .find(|line| line.starts_with(&format!("{id} ")))
        .unwrap_or_else(|| panic!("no row {id} in {block}"));
    row.split("  ")
        .map(str::trim)
        .filter(|cell| !cell.is_empty())
        .collect()
}

/// `ignite check --std <std> <file>`.
fn check_with_std(std: &Path, file: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("check")
        .arg("--std")
        .arg(std)
        .arg(file)
        .output()
        .expect("run ignite check")
}

/// A copy of the directory `from` at `to`, replacing what was there.
fn copy_tree(from: &Path, to: &Path) {
    let _ = std::fs::remove_dir_all(to);
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

/// `source` with `requirement` removed from every `satisfies [..]` clause,
/// and a clause it leaves empty deleted with the space before it; the
/// number of clauses that named it.
fn without_satisfies(source: &str, requirement: &str) -> (String, usize) {
    let mut out = String::new();
    let mut rest = source;
    let mut removed = 0;
    while let Some(at) = rest.find(" satisfies [") {
        let open = at + " satisfies [".len();
        let close = open + rest[open..].find(']').expect("a closed satisfies clause");
        let entries: Vec<&str> = rest[open..close]
            .split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .collect();
        let kept: Vec<&str> = entries
            .iter()
            .copied()
            .filter(|entry| *entry != requirement)
            .collect();
        if kept.len() == entries.len() {
            out.push_str(&rest[..=close]);
        } else {
            removed += 1;
            out.push_str(&rest[..at]);
            if !kept.is_empty() {
                out.push_str(&format!(" satisfies [{}]", kept.join(", ")));
            }
        }
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    (out, removed)
}

/// Row 1 of 19.0, P2 test 1: with `RFC854.RP14_1` (IAC doubling) removed
/// from every `satisfies` of a copy of `std/`, `ignite check` of the copy's
/// Telnet module reports exactly one E090, naming it.
#[test]
fn removing_a_satisfies_from_a_must_is_e090() {
    let std = tmp("row1-std");
    copy_tree(&repository().join("std"), &std);
    let telnet = std.join("protocol/telnet.ignite");
    let unchanged = check_with_std(&std, &telnet);
    assert_eq!(
        unchanged.status.code(),
        Some(0),
        "{}",
        text(&unchanged.stderr)
    );
    assert_eq!(text(&unchanged.stdout), "");
    assert_eq!(text(&unchanged.stderr), "");

    let source = std::fs::read_to_string(&telnet).unwrap();
    let (edited, removed) = without_satisfies(&source, "RFC854.RP14_1");
    assert!(removed > 0, "no satisfies clause names RFC854.RP14_1");
    let (_, left) = without_satisfies(&edited, "RFC854.RP14_1");
    assert_eq!(left, 0);
    std::fs::write(&telnet, edited).unwrap();

    let output = check_with_std(&std, &telnet);
    assert_eq!(output.status.code(), Some(1), "{}", text(&output.stderr));
    assert_eq!(text(&output.stdout), "");
    let stderr = text(&output.stderr);
    let lines: Vec<&str> = stderr.lines().collect();
    assert_eq!(lines.len(), 1, "{stderr}");
    let prefix = format!("{}:", telnet.display());
    let rest = lines[0]
        .strip_prefix(&prefix)
        .unwrap_or_else(|| panic!("{stderr}"));
    let (position, message) = rest
        .split_once(": error[E090]: ")
        .unwrap_or_else(|| panic!("{stderr}"));
    let (line, column) = position
        .split_once(':')
        .unwrap_or_else(|| panic!("{stderr}"));
    assert!(
        line.parse::<u32>().is_ok() && column.parse::<u32>().is_ok(),
        "{stderr}"
    );
    assert!(message.contains("`RFC854.RP14_1`"), "{stderr}");
}

/// Row 2 of 19.0, P2 test 2: the HTTP matrix claims nothing. RFC 9112's
/// chunked coding (section 7.1) and Host (section 3.2) were gaps until story
/// `http-rfc9112-9110-gaps`; one gap of each RFC stays, RFC 9112's octet head
/// (section 2.2) and RFC 9110's Date (section 6.6.1). The rows that story
/// closed are satisfied, each by its fn and a passing test that names it
/// (19.22); since story `http-server-responses` Http chunks a streamed
/// response, and the rows that forbid it a `Transfer-Encoding` are satisfied by
/// `streamed_framing` (19.23).
#[test]
fn http_matrix_lists_the_chunked_and_host_gaps() {
    const TRANSFER_ENCODING: &str = "test \"an application Transfer-Encoding is 500\" passed";
    const CHUNKS: &str = "test \"Http chunks only an HTTP/1.1 response with content\" passed";
    const ABSOLUTE_FORM: &str =
        "test \"an absolute-form target is accepted and its authority replaces Host\" passed";
    const HEAD: &str = "test \"HEAD is answered as GET without content\" passed";
    const CHUNKED: &str = "test \"a chunked request body is decoded\" passed";
    const TRAILER: &str = "test \"trailer fields are checked and discarded and never merged into the header section\" passed";
    const CLOSED: [(&str, &str, &str, &str); 26] = [
        (
            "rfc9112",
            "R2_2_8",
            "checked",
            "test \"a response header name that is not a token is 500\" passed",
        ),
        ("rfc9112", "R6_1_2", "checked", TRANSFER_ENCODING),
        ("rfc9112", "R6_1_4", "checked", TRANSFER_ENCODING),
        ("rfc9112", "R6_1_7", "streamed_framing", CHUNKS),
        ("rfc9112", "R6_1_8", "streamed_framing", CHUNKS),
        ("rfc9112", "R6_1_11", "streamed_framing", CHUNKS),
        ("rfc9112", "R6_2_1", "checked", TRANSFER_ENCODING),
        (
            "rfc9110",
            "R5_3_3",
            "checked",
            "test \"a repeated response field is 500 unless it is a list or Set-Cookie\" passed",
        ),
        (
            "rfc9110",
            "R5_6_1_1_1",
            "checked",
            "test \"an empty element of a response list field is 500\" passed",
        ),
        (
            "rfc9110",
            "R5_6_3_4",
            "checked",
            "test \"BWS in a response's auth-param is 500\" passed",
        ),
        (
            "rfc9110",
            "R5_6_7_2",
            "checked",
            "test \"a response timestamp is an IMF-fixdate with no extra whitespace\" passed",
        ),
        (
            "rfc9110",
            "R5_6_7_3",
            "checked",
            "test \"a response timestamp is an IMF-fixdate with no extra whitespace\" passed",
        ),
        (
            "rfc9112",
            "R3_3",
            "target_too_long",
            "test \"a request-target longer than the head limit is 414\" passed",
        ),
        (
            "rfc9112",
            "R3_2_6",
            "resolve_host",
            "test \"an HTTP/1.1 request has exactly one valid Host or is 400\" passed",
        ),
        ("rfc9112", "R3_2_2_5", "resolve_host", ABSOLUTE_FORM),
        ("rfc9112", "R3_2_2_6", "next_request", ABSOLUTE_FORM),
        ("rfc9110", "R9_1_1", "session", HEAD),
        ("rfc9110", "R9_3_2_1", "session", HEAD),
        ("rfc9112", "R6_1_1", "read_chunked", CHUNKED),
        ("rfc9112", "R7_1_2", "read_chunked", CHUNKED),
        (
            "rfc9112",
            "R6_3_3",
            "coding_status",
            "test \"a Transfer-Encoding Http cannot frame is 400 and an unknown coding 501\" passed",
        ),
        (
            "rfc9112",
            "R7_1_3",
            "chunk_size",
            "test \"a chunked body over the body limit is 413 however many digits its size has\" passed",
        ),
        (
            "rfc9112",
            "R7_1_1_1",
            "chunk_size",
            "test \"chunk extensions are ignored\" passed",
        ),
        ("rfc9112", "R7_1_2_2", "read_chunked", TRAILER),
        ("rfc9112", "R7_1_2_3", "read_chunked", TRAILER),
        ("rfc9110", "R6_5_1_2", "read_chunked", TRAILER),
    ];
    let mut outputs = Vec::new();
    for target in ["rust", "llvm"] {
        let output = std_conformance("http", target);
        let stdout = text(&output.stdout);
        let rfc9112 = block(&stdout, "rfc9112");
        for (rfc, id, section, level) in [
            ("rfc9112", "R2_2_1", "2.2", "MUST"),
            ("rfc9110", "R6_6_1_2", "6.6.1", "MUST"),
            ("rfc9110", "R6_2_5", "6.2", "MUST NOT"),
        ] {
            let row = columns(block(&stdout, rfc), id);
            assert_eq!(row.len(), 6, "{target}: {row:?}");
            assert_eq!(row[1], section, "{target}: {row:?}");
            assert_eq!(row[2], level, "{target}: {row:?}");
            assert_eq!(row[3], "unsupported", "{target}: {row:?}");
            assert_eq!(row[5], "gap", "{target}: {row:?}");
        }
        for (rfc, id, satisfier, test) in CLOSED {
            let row = columns(block(&stdout, rfc), id);
            assert_eq!(row.len(), 6, "{target}: {row:?}");
            assert_eq!(row[3], "satisfied", "{target}: {row:?}");
            assert_eq!(row[5], "pass", "{target}: {row:?}");
            let (satisfiers, tests) = row[4]
                .split_once("; ")
                .unwrap_or_else(|| panic!("{target}: {row:?}"));
            assert!(
                satisfiers.split(", ").any(|name| name == satisfier),
                "{target}: {row:?}"
            );
            assert!(tests.split(", ").any(|t| t == test), "{target}: {row:?}");
        }
        let last = rfc9112.trim_end().lines().last().unwrap_or_default();
        assert_eq!(
            last, "claim conformance of Http to rfc9112: no (1 gaps, 0 failures, 0 inconclusive)",
            "{target}"
        );
        assert!(
            stdout.contains(
                "claim conformance of Http to rfc9110: no (2 gaps, 0 failures, 0 inconclusive)\n"
            ),
            "{target}: {stdout}"
        );
        assert_eq!(
            output.status.code(),
            Some(1),
            "{target}: {stdout}{}",
            text(&output.stderr)
        );
        outputs.push(stdout);
    }
    assert_eq!(outputs[0], outputs[1]);
}

/// The JsonRpc claim of 19.19: one block labelled by the text id, every
/// MUST-class row in scope held or stated, the batch rows unsupported, the
/// three rows that need std.json's decimals (story:json-decimals, 19.20)
/// satisfied, so the claim is yes and exits 0; the same matrix on both
/// targets.
#[test]
fn json_rpc_matrix_is_labelled_by_its_id_and_holds() {
    let mut outputs = Vec::new();
    for target in ["rust", "llvm"] {
        let output = std_conformance("jsonrpc", target);
        let stdout = text(&output.stdout);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{target}: {stdout}{}",
            text(&output.stderr)
        );
        assert!(
            stdout.starts_with(
                "jsonrpc-2.0 JSON-RPC 2.0 Specification, protocol JsonRpc, parties Server Any, sections all\n"
            ),
            "{target}: {stdout}"
        );
        let block = block(&stdout, "jsonrpc-2.0");
        assert_eq!(
            block.trim_end().lines().last().unwrap_or_default(),
            "claim conformance of JsonRpc to jsonrpc-2.0: yes (0 gaps, 0 failures, 0 inconclusive)",
            "{target}"
        );
        for id in ["R6_2", "R6_3", "R6_4", "R6_5"] {
            let row = columns(block, id);
            assert_eq!(row[1], "6", "{target}: {row:?}");
            assert_eq!(row[3], "unsupported", "{target}: {row:?}");
        }
        for id in ["R4_6", "R4_1_1", "R5_1", "R5_9", "R5_10", "R6_7"] {
            let row = columns(block, id);
            assert_eq!(row[3], "satisfied", "{target}: {row:?}");
            assert_eq!(row[5], "pass", "{target}: {row:?}");
        }
        for (id, test) in [
            ("R4_6", "a request whose id is a fraction or has 20 digits"),
            (
                "R4_1_1",
                "a notification with a fractional or exponent param",
            ),
            ("R5_9", "a request with a fractional param"),
        ] {
            let row = columns(block, id);
            assert!(row[4].contains(test), "{target}: {row:?}");
        }
        assert_eq!(columns(block, "R4_1")[5], "out of scope", "{target}");
        outputs.push(stdout);
    }
    assert_eq!(outputs[0], outputs[1]);
}

/// Row 4 of 19.0: both std protocol modules carry a status for every
/// MUST-class requirement in scope.
#[test]
fn std_protocol_modules_check_silently() {
    for name in ["telnet", "http", "jsonrpc"] {
        let output = Command::new(env!("CARGO_BIN_EXE_ignite"))
            .arg("check")
            .arg(repository().join(format!("std/protocol/{name}.ignite")))
            .output()
            .expect("run ignite check");
        assert_eq!(
            output.status.code(),
            Some(0),
            "{name}: {}",
            text(&output.stderr)
        );
        assert_eq!(text(&output.stdout), "", "{name}");
        assert_eq!(text(&output.stderr), "", "{name}");
    }
}

/// The digests of the five `Spec` constants (19.9).
const SPEC_DIGESTS: [&str; 5] = [
    "f9cb6d4a7d7a67e5a1ac5ae3c2962b1183fb26119632a1617f3c8379820dfb10",
    "c8c4744c9fa7650c0ae78d140f560b26e26b2935391e1ff624476c5fdbf4e03a",
    "4d2ef5878b0b88742c860fc777d6d91ad61580633fe277eae06410a6eee04bdf",
    "09ec9ad4d1664623adcb298577e3fb9cd5c285f1bbe6b24d8c715b29f218e721",
    "ce82219c272374005115c909244a79e587687df3858edca4112b8563c02ccf19",
];

/// Row 5 of 19.0: the Rust that `ignite emit` prints and every `.ll` file
/// that `ignite build --target llvm` writes for the two examples hold no
/// `Spec` digest and no requirement text.
#[test]
fn emitted_programs_carry_no_catalogue() {
    for digest in SPEC_DIGESTS {
        let catalogues = ["rfc854", "rfc855", "rfc1143", "rfc9110", "rfc9112"];
        assert!(
            catalogues.iter().any(|name| {
                std::fs::read_to_string(repository().join(format!("std/rfc/{name}.ignite")))
                    .unwrap()
                    .contains(&format!("digest = \"sha256:{digest}\""))
            }),
            "no catalogue has the digest {digest}"
        );
    }
    let runtime = common::runtime_archive();
    for example in ["todo/todo.ignite", "terminal/terminal.ignite"] {
        let file = repository().join("examples").join(example);
        let emit = Command::new(env!("CARGO_BIN_EXE_ignite"))
            .arg("emit")
            .arg(&file)
            .output()
            .expect("run ignite emit");
        assert_eq!(
            emit.status.code(),
            Some(0),
            "{example}: {}",
            text(&emit.stderr)
        );
        let mut emitted = vec![(format!("{example}: ignite emit"), text(&emit.stdout))];
        assert!(!emitted[0].1.is_empty(), "{example}");

        let out = tmp(&format!("row5-{}", example.replace('/', "-")));
        let _ = std::fs::remove_dir_all(&out);
        let build = Command::new(env!("CARGO_BIN_EXE_ignite"))
            .args(["build", "--target", "llvm"])
            .arg(&file)
            .arg("--out")
            .arg(&out)
            .arg("--runtime")
            .arg(&runtime)
            .output()
            .expect("run ignite build");
        assert_eq!(
            build.status.code(),
            Some(0),
            "{example}: {}",
            text(&build.stderr)
        );
        let mut pending = vec![out.clone()];
        while let Some(path) = pending.pop() {
            for entry in std::fs::read_dir(&path).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    pending.push(path);
                } else if path.extension().is_some_and(|e| e == "ll") {
                    let content = std::fs::read_to_string(&path).unwrap();
                    emitted.push((path.display().to_string(), content));
                }
            }
        }
        let names: Vec<&str> = emitted.iter().map(|(name, _)| name.as_str()).collect();
        for file in ["std.ll", "program.ll"] {
            assert!(
                names.iter().any(|name| name.ends_with(&format!("/{file}"))),
                "{example}: {names:?}"
            );
        }
        assert!(
            names
                .iter()
                .any(|name| name.contains("/main_") && name.ends_with(".ll")),
            "{example}: {names:?}"
        );
        for (name, content) in &emitted {
            for digest in SPEC_DIGESTS {
                assert!(!content.contains(digest), "{name} holds {digest}");
            }
            assert!(
                !content.contains("parse and decode the chunked transfer coding"),
                "{name} holds the text of RFC9112.R7_1_2"
            );
        }
    }
}

/// Row 3 of 19.0, P2 test 3 (19.14 item 16): the RFC 854 claim holds for
/// an NVT that refuses every option. The four rows that were gaps in slice
/// 15 are satisfied and pass, each on its satisfying fn and its verifying
/// test: `RP5_3` (a key sends a partial line), `RP5_4` (IAC GA after the
/// prompt), `RP10_2` (IAC IP followed by the Synch) and `RP12_4` (the Synch
/// discards the data before its Data Mark). No row is a gap. The RFC 855
/// claim holds, and its negotiation rows `RP2_3` (IAC SE ends a
/// subnegotiation) and `RP2_4` (refusal by WON'T or DON'T) pass on
/// `decode_within` and both its tests.
#[test]
fn telnet_matrix_claims_rfc854_for_an_nvt_that_refuses_every_option() {
    const CLOSED: [(&str, &str, &str); 4] = [
        (
            "RP5_3",
            "apply_key",
            "test \"Ctrl+D on a partial line sends it without a line end\" passed",
        ),
        (
            "RP5_4",
            "answer_line",
            "test \"the server sends IAC GA after each prompt and the AYT answer\" passed",
        ),
        (
            "RP10_2",
            "interrupt",
            "test \"an interrupt is IAC IP followed by the Synch\" passed",
        ),
        (
            "RP12_4",
            "session",
            "test \"a Synch discards the data before its Data Mark\" passed",
        ),
    ];
    let mut outputs = Vec::new();
    for target in ["rust", "llvm"] {
        let output = std_conformance("telnet", target);
        let stdout = text(&output.stdout);
        for claim in [
            "claim conformance of Telnet to rfc854: yes (0 gaps, 0 failures, 0 inconclusive)\n",
            "claim conformance of Telnet to rfc855: yes (0 gaps, 0 failures, 0 inconclusive)\n",
        ] {
            assert!(
                stdout.contains(claim),
                "{target}: {claim}{stdout}{}",
                text(&output.stderr)
            );
        }
        let rfc854 = block(&stdout, "rfc854");
        for (id, satisfier, test) in CLOSED {
            let row = columns(rfc854, id);
            assert_eq!(row.len(), 6, "{target}: {row:?}");
            assert_eq!(row[3], "satisfied", "{target}: {row:?}");
            assert_eq!(row[5], "pass", "{target}: {row:?}");
            let (satisfiers, tests) = row[4]
                .split_once("; ")
                .unwrap_or_else(|| panic!("{target}: {row:?}"));
            assert!(
                satisfiers.split(", ").any(|name| name == satisfier),
                "{target}: {row:?}"
            );
            assert!(tests.split(", ").any(|t| t == test), "{target}: {row:?}");
        }
        let gaps: Vec<&str> = rfc854
            .lines()
            .filter(|line| line.starts_with('R') && line.ends_with("  gap"))
            .filter_map(|line| line.split(' ').next())
            .collect();
        assert!(gaps.is_empty(), "{target}: {rfc854}");
        let rfc855 = block(&stdout, "rfc855");
        for id in ["RP2_3", "RP2_4"] {
            let row = columns(rfc855, id);
            assert_eq!(row.len(), 6, "{target}: {row:?}");
            assert_eq!(row[3], "satisfied", "{target}: {row:?}");
            assert_eq!(row[5], "pass", "{target}: {row:?}");
            let (satisfiers, tests) = row[4]
                .split_once("; ")
                .unwrap_or_else(|| panic!("{target}: {row:?}"));
            assert!(
                satisfiers.split(", ").any(|name| name == "decode_within"),
                "{target}: {row:?}"
            );
            for test in [
                "test \"a subnegotiation is skipped up to IAC SE\" passed",
                "test \"DO is answered WONT and WILL is answered DONT\" passed",
            ] {
                assert!(tests.split(", ").any(|t| t == test), "{target}: {row:?}");
            }
        }
        assert_eq!(output.status.code(), Some(0), "{target}: {stdout}");
        outputs.push(stdout);
    }
    assert_eq!(outputs[0], outputs[1]);
}

// U6's adversary: the claims of `std/protocol/telnet.ignite` and
// `http.ignite` against their RFC sentences, and rows 1 and 5 of 19.0 at
// their edges.

/// `ignite test <fixture> [filter] --target <target>`, its builds under
/// `tmp`, the Rust builds in the shared cargo target directory.
fn adversary_inline(name: &str, filter: Option<&str>, target: &str) -> Output {
    let _guard = BUILDS.lock().unwrap_or_else(|e| e.into_inner());
    let mut command = Command::new(env!("CARGO_BIN_EXE_ignite"));
    command.arg("test").arg(fixture(name));
    if let Some(filter) = filter {
        command.arg(filter);
    }
    command
        .args(["--target", target, "--out"])
        .arg(tmp(&format!("inline-{name}-{target}")));
    if target == "rust" {
        command.arg("--target-dir").arg(tmp("cargo"));
    } else {
        command.arg("--runtime").arg(common::runner_archive());
    }
    command.output().expect("run ignite test")
}

/// The inline test `test` of fixture `name` passes on both targets.
fn assert_inline_passes(name: &str, test: &str) {
    let line = format!("test tests.conformance.{name}/{test} ... ok\n");
    let mut failed = Vec::new();
    for target in ["rust", "llvm"] {
        let output = adversary_inline(name, Some(test), target);
        let stdout = text(&output.stdout);
        if !stdout.contains(&line) || output.status.code() != Some(0) {
            failed.push(format!(
                "{target}: exit {:?}\n{stdout}{}",
                output.status.code(),
                text(&output.stderr)
            ));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// RFC 854 p11, `RFC854.RP11_3`, and p14, `RP14_1`, which `render`,
/// `encode_input`, `answer_line` and `session` claim: a reply line with a
/// bare CR, a bare LF and the text made of the byte 255, and the prompt
/// `"\r>\n"` both before and after a reply, reach the client as CR NUL and
/// CR LF, with no byte 255, on both targets. U6's own pipe test closes on
/// its first line, so the prompt that `answer_line` sends after a reply is
/// covered only here.
#[test]
fn adversary_telnet_replies_and_prompts_are_nvt_data_on_both_targets() {
    assert_inline_passes(
        "adversary_std_telnet",
        "adversary: replies and the prompt after a reply are NVT data",
    );
}

/// `source` with `requirement` removed from every `<word> [..]` clause,
/// and a clause it leaves empty deleted with the space before it; the
/// number of clauses that named it.
fn adversary_without(source: &str, word: &str, requirement: &str) -> (String, usize) {
    let opening = format!(" {word} [");
    let mut out = String::new();
    let mut rest = source;
    let mut removed = 0;
    while let Some(at) = rest.find(&opening) {
        let open = at + opening.len();
        let close = open + rest[open..].find(']').expect("a closed clause");
        let entries: Vec<&str> = rest[open..close]
            .split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .collect();
        let kept: Vec<&str> = entries
            .iter()
            .copied()
            .filter(|entry| *entry != requirement)
            .collect();
        if kept.len() == entries.len() {
            out.push_str(&rest[..=close]);
        } else {
            removed += 1;
            out.push_str(&rest[..at]);
            if !kept.is_empty() {
                out.push_str(&format!("{opening}{}]", kept.join(", ")));
            }
        }
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    (out, removed)
}

/// Row 1 of 19.0 at its edges. 19.7: the E090 message is "requirement
/// `RFC9112.R7_1_2` (MUST, section 7.1) is in scope of `Http` and has no
/// status; add `satisfies`, `unsupported` or `not_applicable`", at "the
/// spec's entry in P's `conforms`". With `RFC854.RP14_1` gone from every
/// `satisfies`, and then also from every `verifies`, stderr is that one
/// line at the `RFC854` of `protocol Telnet conforms [RFC854, RFC855]`.
#[test]
fn adversary_row1_e090_is_the_contract_line_at_the_conforms_entry() {
    let std = tmp("adversary-row1-std");
    copy_tree(&repository().join("std"), &std);
    let telnet = std.join("protocol/telnet.ignite");
    let source = std::fs::read_to_string(&telnet).unwrap();
    let (line, column) = source
        .lines()
        .enumerate()
        .find_map(|(i, line)| {
            line.strip_prefix("protocol Telnet conforms [")
                .map(|_| (i + 1, "protocol Telnet conforms [".len() + 1))
        })
        .expect("the conforms line of Telnet");
    let want = format!(
        "{}:{line}:{column}: error[E090]: requirement `RFC854.RP14_1` (MUST, section p14) is in scope of `Telnet` and has no status; add `satisfies`, `unsupported` or `not_applicable`\n",
        telnet.display()
    );

    let (satisfied_nowhere, removed) = adversary_without(&source, "satisfies", "RFC854.RP14_1");
    assert_eq!(removed, 3, "decode_within, render and encode_input");
    std::fs::write(&telnet, &satisfied_nowhere).unwrap();
    let output = check_with_std(&std, &telnet);
    assert_eq!(text(&output.stderr), want);
    assert_eq!(text(&output.stdout), "");
    assert_eq!(output.status.code(), Some(1));

    let (verified_nowhere, removed) =
        adversary_without(&satisfied_nowhere, "verifies", "RFC854.RP14_1");
    assert!(removed >= 4, "{removed} verifies clauses named RP14_1");
    std::fs::write(&telnet, &verified_nowhere).unwrap();
    let output = check_with_std(&std, &telnet);
    assert_eq!(text(&output.stderr), want);
    assert_eq!(text(&output.stdout), "");
    assert_eq!(output.status.code(), Some(1));
}

/// Row 5 of 19.0 for a program that imports `std.rfc` directly and names
/// `Level`, `Requirement` and `Spec`, but no catalogue.
#[test]
fn adversary_row5_a_program_importing_std_rfc_carries_no_catalogue() {
    assert_carries_no_catalogue("adversary_imports_rfc", "rfc-types");
}

/// Row 5 of 19.0 for a program whose own protocol `conforms [RFC855]`,
/// the import of `RFC855` used by that clause only. D2: "Clauses are no
/// expressions: no backend reads them and no reachability walk starts at
/// them", and "an unreferenced constant leaves no text or debug entry on
/// either target". LLVM holds none; the generated `ignite-std` of
/// `ignite emit` holds all of `std.rfc.rfc855`, because D2 also makes "a
/// program's `use` lines" roots on Rust.
#[test]
#[ignore = "fixed on branch fix/rustgen-builtin-shadowing (9bbd4b2, a clause-only catalogue import emits none of its constants), not merged into s15/claims"]
fn adversary_row5_a_program_conforming_to_a_std_catalogue_carries_none() {
    assert_carries_no_catalogue("adversary_conforms_rfc855", "rfc855-claim");
}

/// `ignite emit` of fixture `name` and every `.ll` of its `ignite build
/// --target llvm` hold none of the five digests of 19.9 and no requirement
/// text of RFC 854, 855 or 9112.
fn assert_carries_no_catalogue(name: &str, out: &str) {
    let file = fixture(name);
    let check = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("check")
        .arg(&file)
        .output()
        .expect("run ignite check");
    assert_eq!(check.status.code(), Some(0), "{}", text(&check.stderr));
    assert_eq!(text(&check.stderr), "");
    let emit = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("emit")
        .arg(&file)
        .output()
        .expect("run ignite emit");
    assert_eq!(emit.status.code(), Some(0), "{}", text(&emit.stderr));
    let mut emitted = vec![("ignite emit".to_string(), text(&emit.stdout))];
    assert!(!emitted[0].1.is_empty());
    let out = tmp(&format!("adversary-row5-{out}"));
    let _ = std::fs::remove_dir_all(&out);
    let build = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .args(["build", "--target", "llvm"])
        .arg(&file)
        .arg("--out")
        .arg(&out)
        .arg("--runtime")
        .arg(common::runtime_archive())
        .output()
        .expect("run ignite build");
    assert_eq!(build.status.code(), Some(0), "{}", text(&build.stderr));
    let mut pending = vec![out.clone()];
    while let Some(path) = pending.pop() {
        for entry in std::fs::read_dir(&path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|e| e == "ll") {
                let content = std::fs::read_to_string(&path).unwrap();
                emitted.push((path.display().to_string(), content));
            }
        }
    }
    assert!(
        emitted.iter().any(|(name, _)| name.ends_with("/std.ll")),
        "{:?}",
        emitted.iter().map(|(name, _)| name).collect::<Vec<_>>()
    );
    for (name, content) in &emitted {
        for digest in SPEC_DIGESTS {
            assert!(!content.contains(digest), "{name} holds {digest}");
        }
        for requirement in [
            "parse and decode the chunked transfer coding",
            "only the IAC need be doubled to be sent as data",
            "Telnet Option Specifications",
            "either party may refuse to pursue further subnegotiation",
        ] {
            assert!(!content.contains(requirement), "{name} holds {requirement}");
        }
    }
}

/// RFC 9112 6.3, `RFC9112.R6_3_4` and `R6_3_5`, claimed satisfied by
/// `next_request` and `parse_length`: `Content-Length: 3<U+00A0>` (also
/// U+0085, U+3000) is answered 400. Found red while `parse_head` trimmed
/// field values with the Unicode `trim`; it now trims SP and HTAB only
/// (`trim_ows`, RFC 9110 OWS).
#[test]
fn adversary_http_content_length_beside_a_unicode_space_is_400() {
    assert_inline_passes(
        "adversary_std_http",
        "adversary: a Content-Length beside a Unicode space is 400",
    );
}

/// RFC 9112 9.6, `RFC9112.R9_6_6` and `R9_6_7`, claimed satisfied by
/// `reject` and `session`: after a response whose application headers
/// carry `Connection: close`, the server closes the connection and answers
/// no further pipelined request. Found red while `session` looked only at
/// the request; it now also asks `response_closes`.
#[test]
fn adversary_http_a_response_with_the_close_option_ends_the_connection() {
    assert_inline_passes(
        "adversary_std_http",
        "adversary: a response with the close option ends the connection",
    );
}

/// RFC 9112 6.1, `RFC9112.R6_1_4`: an application's `Transfer-Encoding:
/// gzip` was sent beside Http's Content-Length on a kept connection. Found
/// while R6_1_4 was `not_applicable`; restated `unsupported`, then closed
/// by story `http-rfc9112-9110-gaps`: `checked` answers 500 to a response
/// with an application `Transfer-Encoding`, so Http applies no transfer
/// coding (also R6_1_2, R6_1_7, R6_1_8, R6_1_11 and R6_2_1).
#[test]
fn adversary_http_a_response_with_a_transfer_coding_ends_the_connection() {
    assert_inline_passes(
        "adversary_std_http",
        "adversary: a response with a transfer coding other than chunked ends the connection",
    );
}

/// RFC 9112 2.2, `RFC9112.R2_2_1`: the head is decoded to text before it
/// is parsed, so the obs-text octet 0xE9 reaches the application as
/// U+FFFD, three octets. Found while `parse_head` and `read_head` claimed
/// it; it is now `unsupported`, a gap, and this test stays ignored until
/// the gap is closed.
#[test]
#[ignore = "gap RFC9112.R2_2_1 (unsupported in http.ignite): the head is parsed as lossy UTF-8 text, octet 0xE9 reaches the application as U+FFFD; story http-rfc9112-9110-gaps"]
fn adversary_http_an_obs_text_octet_reaches_the_application_as_one_octet() {
    assert_inline_passes(
        "adversary_std_http",
        "adversary: an obs-text octet reaches the application as one octet",
    );
}
