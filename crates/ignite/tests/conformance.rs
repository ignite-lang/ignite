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
