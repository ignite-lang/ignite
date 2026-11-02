//! `ignite test` end to end on both targets (`docs/slice.md` section 12):
//! the fixtures in `tests/fixtures/tests` are built into test executables
//! and run; the report on stdout and the exit status are checked.
//!
//! Every test runs its two targets on two threads ([`on_both_targets`]),
//! and the targets have separate output directories and locks, so a Rust
//! build (cargo) and an LLVM build (clang) overlap; the report of
//! `basics.ignite` is produced once per target and shared.

use crate::common;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

/// One `ignite test` per target at a time: the invocations of a target
/// share its output directory and, on the Rust target, cargo's target
/// directory.
static BUILDS: [Mutex<()>; 2] = [Mutex::new(()), Mutex::new(())];

const TARGETS: [&str; 2] = ["rust", "llvm"];

fn target_index(target: &str) -> usize {
    TARGETS
        .iter()
        .position(|t| *t == target)
        .unwrap_or_else(|| panic!("unknown target {target}"))
}

fn fixture(name: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/tests")
        .join(name)
        .display()
        .to_string()
}

fn out_dir() -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join("ignite-test")
}

struct Run {
    status: i32,
    stdout: String,
    stderr: String,
}

/// `ignite test <args> --target <target>`.
fn ignite_test(target: &str, args: &[&str]) -> Run {
    let _guard = BUILDS[target_index(target)]
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let out = out_dir().join(target);
    let mut command = Command::new(env!("CARGO_BIN_EXE_ignite"));
    command
        .arg("test")
        .args(args)
        .args(["--target", target, "--out"])
        .arg(&out)
        .arg("--runtime")
        .arg(runner_runtime());
    if target == "rust" {
        command.arg("--target-dir").arg(out_dir().join("cargo"));
    }
    let output = command.output().expect("run ignite test");
    Run {
        status: output.status.code().expect("ignite test exits"),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// `f` once per target, the two on their own threads; a failing target
/// fails the test with its own panic message.
fn on_both_targets(f: impl Fn(&str) + Sync) {
    std::thread::scope(|scope| {
        let handles: Vec<_> = TARGETS
            .iter()
            .map(|target| scope.spawn(|| f(target)))
            .collect();
        let failed = handles
            .into_iter()
            .map(|handle| handle.join())
            .filter(Result::is_err)
            .count();
        assert_eq!(failed, 0, "{failed} target(s) failed");
    });
}

/// The report without the time of the summary line.
fn without_time(report: &str) -> String {
    report
        .lines()
        .map(|line| match line.find("; finished in ") {
            Some(at) => &line[..at],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The run of `basics.ignite` on `target`, once per test binary: four
/// tests read it.
fn basics(target: &str) -> &'static Run {
    static RUNS: [OnceLock<Run>; 2] = [OnceLock::new(), OnceLock::new()];
    RUNS[target_index(target)].get_or_init(|| ignite_test(target, &[&fixture("basics.ignite")]))
}

#[test]
fn counts_results_and_failures() {
    on_both_targets(|target| {
        let run = basics(target);
        let out = &run.stdout;
        assert_eq!(run.status, 1, "{target}: {out}\n{}", run.stderr);
        assert!(out.starts_with("running 12 tests\n"), "{target}: {out}");
        for line in [
            "test fixtures.basics/arithmetic passes ... ok",
            "test fixtures.basics/int differs ... FAILED",
            "test fixtures.basics/exit code 3 ... FAILED (exited with code 3)",
            "test fixtures.basics/index out of range panics ... FAILED",
            "test fixtures.basics/output hidden on success ... ok",
            "test fixtures.basics/tcp echo through a task ... ok",
            "test result: FAILED. 3 passed; 9 failed; 0 filtered out; finished in ",
        ] {
            assert!(out.contains(line), "{target}: no `{line}` in\n{out}");
        }
        assert!(
            out.contains("failures:\n    fixtures.basics/int differs\n"),
            "{target}: {out}"
        );
    });
}

#[test]
fn assert_and_try_reports() {
    let path = fixture("basics.ignite");
    on_both_targets(|target| {
        let out = &basics(target).stdout;
        for report in [
            format!(
                "---- fixtures.basics/int differs stderr ----\nerror: assert failed at {path}:33:3: 1 + 1 == 3\n  left: 2\n  right: 3\n"
            ),
            format!(
                "error: assert failed at {path}:37:3: \" ab \".trim() == \"ab \"\n  left: ab\n  right: ab \n"
            ),
            format!(
                "error: assert failed at {path}:41:3: Point {{ x = 1, y = 2 }} == Point {{ x = 1, y = 3 }}\n  left: (1, 2)\n  right: (1, 3)\n"
            ),
            format!("error: assert failed at {path}:46:3: n < 3\n  message: n is 4\n"),
            format!("error: ? failed at {path}:50:21: Err(not a number: x)\n"),
            format!("error: ? failed at {path}:55:25: None\n"),
            format!(
                "---- fixtures.basics/index out of range panics stderr ----\nerror: index failed at {path}:61:10: index 5 out of range for length 2\n"
            ),
        ] {
            assert!(out.contains(&report), "{target}: no\n{report}\nin\n{out}");
        }
    });
}

#[test]
fn captured_output_only_for_failures() {
    on_both_targets(|target| {
        let out = &basics(target).stdout;
        assert!(
            out.contains(
                "---- fixtures.basics/output shown on failure stdout ----\ncaptured out\n"
            ),
            "{target}: {out}"
        );
        assert!(!out.contains("hidden out"), "{target}: {out}");
        assert!(
            !out.contains("---- fixtures.basics/exit code 3 stdout"),
            "{target}: {out}"
        );
    });
}

#[test]
fn both_targets_print_the_same_report() {
    let reports: Vec<String> = TARGETS
        .iter()
        .map(|target| {
            without_time(&basics(target).stdout)
                .lines()
                .filter(|line| !line.starts_with("error: panic at "))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect();
    assert_eq!(reports[0], reports[1]);
}

#[test]
fn filter_exact_and_list() {
    let file = fixture("basics.ignite");
    on_both_targets(|target| {
        let list = ignite_test(target, &[&file, "differs", "--list"]);
        assert_eq!(list.status, 0, "{target}: {}", list.stderr);
        assert_eq!(
            list.stdout,
            "fixtures.basics/int differs: test\nfixtures.basics/text differs: test\nfixtures.basics/record differs: test\n3 tests\n",
            "{target}"
        );
        let exact = ignite_test(target, &[&file, "fixtures.basics/int differs", "--exact"]);
        assert_eq!(exact.status, 1, "{target}: {}", exact.stdout);
        assert!(exact.stdout.starts_with("running 1 test\n"), "{target}");
        assert!(
            exact
                .stdout
                .contains("test result: FAILED. 0 passed; 1 failed; 11 filtered out;"),
            "{target}: {}",
            exact.stdout
        );
        let passing = ignite_test(target, &[&file, "passes"]);
        assert_eq!(passing.status, 0, "{target}: {}", passing.stdout);
        assert_eq!(
            without_time(&passing.stdout),
            "running 1 test\ntest fixtures.basics/arithmetic passes ... ok\n\ntest result: ok. 1 passed; 0 failed; 11 filtered out",
            "{target}"
        );
        let none = ignite_test(target, &[&file, "no such test"]);
        assert_eq!(none.status, 0, "{target}");
        assert_eq!(
            without_time(&none.stdout),
            "running 0 tests\n\ntest result: ok. 0 passed; 0 failed; 12 filtered out",
            "{target}"
        );
    });
}

/// Programs with different packages share one test executable: one list,
/// one count, the tests in command-line order of their files.
#[test]
fn files_share_one_executable() {
    let (basics, slow) = (fixture("basics.ignite"), fixture("slow.ignite"));
    on_both_targets(|target| {
        let list = ignite_test(target, &[&slow, &basics, "fa", "--list"]);
        assert_eq!(list.status, 0, "{target}: {}", list.stderr);
        assert_eq!(
            list.stdout,
            "fixtures.slow/fast: test\nfixtures.basics/try on err fails: test\nfixtures.basics/try on none fails: test\nfixtures.basics/output shown on failure: test\n4 tests\n",
            "{target}"
        );
    });
}

/// Two programs with one module path (`main`, no `package` line) in one
/// invocation cannot share an executable (the runner names a test
/// `main/<name>`): each runs its own tests in an executable of its own,
/// not the executable built last. The second round finds both projects
/// built in the shared cargo target directory, unchanged.
#[test]
fn each_unnamed_file_runs_its_own_tests() {
    let (a, b) = (fixture("unnamed_a.ignite"), fixture("unnamed_b.ignite"));
    on_both_targets(|target| {
        for _round in 0..2 {
            let run = ignite_test(target, &[&a, &b]);
            assert_eq!(run.status, 0, "{target}: {}\n{}", run.stdout, run.stderr);
            assert_eq!(
                without_time(&run.stdout),
                "running 1 test\ntest main/first file ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 filtered out\nrunning 2 tests\ntest main/second file ... ok\ntest main/second file again ... ok\n\ntest result: ok. 2 passed; 0 failed; 0 filtered out",
                "{target}"
            );
        }
    });
}

#[test]
fn timeout_kills_a_test() {
    on_both_targets(|target| {
        let run = ignite_test(target, &[&fixture("slow.ignite"), "--timeout", "1"]);
        assert_eq!(run.status, 1, "{target}: {}\n{}", run.stdout, run.stderr);
        assert!(
            run.stdout
                .contains("test fixtures.slow/never ends ... FAILED (timed out after 1 s)\n"),
            "{target}: {}",
            run.stdout
        );
        assert!(
            run.stdout
                .contains("test result: FAILED. 1 passed; 1 failed; 0 filtered out;"),
            "{target}: {}",
            run.stdout
        );
    });
}

#[test]
fn compile_error_exits_2() {
    on_both_targets(|target| {
        let run = ignite_test(target, &[&fixture("broken.ignite")]);
        assert_eq!(run.status, 2, "{target}: {}", run.stderr);
        assert!(
            run.stderr.contains("error[E040]"),
            "{target}: {}",
            run.stderr
        );
        assert!(run.stdout.is_empty(), "{target}: {}", run.stdout);
    });
}

#[test]
fn std_module_as_the_file() {
    let std = fixture("std");
    on_both_targets(|target| {
        let run = ignite_test(target, &[&fixture("std/greet.ignite"), "--std", &std]);
        assert_eq!(run.status, 0, "{target}: {}\n{}", run.stdout, run.stderr);
        assert_eq!(
            without_time(&run.stdout),
            "running 1 test\ntest std.greet/greets by name ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 filtered out",
            "{target}"
        );
    });
}

/// Slice 11b: the four target bugs of `docs/friction.md` rows 32 to 35
/// (`fixtures/tests/targets.ignite`, one test per row) pass on both
/// targets: a mutating call through an indexed receiver writes the
/// element, a user enum is a `Map` key, `Exposure` is a value type, and
/// user types named `Box`, `Vec`, `String` or with variants `Some`,
/// `None`, `Ok`, `Err` build.
#[test]
fn target_bugs_of_slice_11b_are_fixed() {
    on_both_targets(|target| {
        let run = ignite_test(target, &[&fixture("targets.ignite")]);
        assert_eq!(run.status, 0, "{target}: {}\n{}", run.stdout, run.stderr);
        assert!(
            run.stdout.starts_with("running 4 tests\n"),
            "{target}: {}",
            run.stdout
        );
        for row in ["32", "33", "34", "35"] {
            assert!(
                run.stdout
                    .contains(&format!("test fixtures.targets/row {row}: ")),
                "{target}: no row {row} in\n{}",
                run.stdout
            );
        }
        assert!(
            run.stdout
                .contains("test result: ok. 4 passed; 0 failed; 0 filtered out;"),
            "{target}: {}",
            run.stdout
        );
    });
}

/// The substrate builtin `Clock.monotonic` of slice 12 (`docs/slice.md`
/// 16.13) under `ignite test` on both targets.
#[test]
fn substrate_builtins_run_on_both_targets() {
    on_both_targets(|target| {
        let run = ignite_test(target, &[&fixture("builtins.ignite")]);
        assert_eq!(run.status, 0, "{target}: {}\n{}", run.stdout, run.stderr);
        assert!(
            run.stdout.starts_with("running 1 test\n"),
            "{target}: {}",
            run.stdout
        );
        assert!(
            run.stdout
                .contains("test result: ok. 1 passed; 0 failed; 0 filtered out;"),
            "{target}: {}",
            run.stdout
        );
    });
}

/// `Endpoint.of` on both targets (`docs/slice.md` 16.13, 16.9): the
/// `prober` of `fixtures/tests/builtins.ignite` fails without a `--ref`
/// binding and prints the endpoint of one.
#[test]
fn endpoint_of_reads_the_ref_binding_on_both_targets() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tests/builtins.ignite");
    std::thread::scope(|scope| {
        let handles: Vec<_> = ["rust", "llvm"]
            .into_iter()
            .map(|target| {
                let source = &source;
                scope.spawn(move || {
                    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
                        .join(format!("builtins-{target}"));
                    let _ = std::fs::remove_dir_all(&dir);
                    let mut build = Command::new(env!("CARGO_BIN_EXE_ignite"));
                    build
                        .arg("build")
                        .arg(source)
                        .args(["--target", target, "--out"])
                        .arg(dir.join("out"));
                    if target == "llvm" {
                        build.arg("--runtime").arg(common::runtime_archive());
                    } else {
                        build.arg("--target-dir").arg(out_dir().join("cargo"));
                    }
                    let output = build.output().expect("run ignite build");
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    assert!(
                        output.status.success(),
                        "{target}: {stdout}\n{}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                    let prober = stdout
                        .lines()
                        .filter_map(|line| line.strip_prefix("built "))
                        .find(|path| path.ends_with("/prober"))
                        .unwrap_or_else(|| panic!("{target}: no prober in\n{stdout}"));
                    let unbound = Command::new(prober).output().unwrap();
                    assert_eq!(unbound.status.code(), Some(1), "{target}");
                    assert_eq!(
                        String::from_utf8_lossy(&unbound.stderr),
                        "error: no binding for Probes.server\n",
                        "{target}"
                    );
                    assert!(unbound.stdout.is_empty(), "{target}");
                    let bound = Command::new(prober)
                        .args(["--ref", "Probes.server=127.0.0.1:4242"])
                        .output()
                        .unwrap();
                    assert_eq!(
                        bound.status.code(),
                        Some(0),
                        "{target}: {}",
                        String::from_utf8_lossy(&bound.stderr)
                    );
                    assert_eq!(
                        String::from_utf8_lossy(&bound.stdout),
                        "127.0.0.1:4242\n",
                        "{target}"
                    );
                    let malformed = Command::new(prober)
                        .args(["--ref", "Probes.server"])
                        .output()
                        .unwrap();
                    assert_eq!(malformed.status.code(), Some(2), "{target}");
                })
            })
            .collect();
        let failed = handles
            .into_iter()
            .map(|h| h.join())
            .filter(Result::is_err)
            .count();
        assert_eq!(failed, 0, "{failed} target(s) failed");
    });
}

/// The newest `libignite_rt-*.a` beside the test executable that has the
/// test runner. This crate depends on `ignite-rt` with the feature
/// `runner`, but a `cargo build -p ignite-rt` into the same target
/// directory leaves a newer archive without it.
fn runner_runtime() -> PathBuf {
    let exe = std::env::current_exe().expect("test executable path");
    let deps = exe.parent().expect("test executable directory");
    let mut archives: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(deps)
        .expect("read the deps directory")
        .filter_map(Result::ok)
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.starts_with("libignite_rt-") && name.ends_with(".a")
        })
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .collect();
    archives.sort();
    let symbol = b"ignite_rt_test_main";
    archives
        .into_iter()
        .rev()
        .map(|(_, path)| path)
        .find(|path| {
            std::fs::read(path)
                .is_ok_and(|bytes| bytes.windows(symbol.len()).any(|window| window == symbol))
        })
        .unwrap_or_else(common::runtime_archive)
}

/// `docs/vision.md` T6: `+`, `-`, `*`, unary `-`, `/` and `%` on `Int` and
/// `Byte` fail the test with one report, the same on both targets in dev
/// and in release.
#[test]
fn overflow_fails_the_same_everywhere() {
    let file = fixture("overflow.ignite");
    let reports: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());
    on_both_targets(|target| {
        for profile in [&[][..], &["--release"][..]] {
            let mut args = vec![file.as_str()];
            args.extend(profile.iter().copied());
            let run = ignite_test(target, &args);
            let label = format!("{target} {profile:?}");
            assert_eq!(run.status, 1, "{label}: {}\n{}", run.stdout, run.stderr);
            let expected = [
                ("int add", "+", 16, "9223372036854775807 + 1"),
                ("int sub", "-", 21, "-9223372036854775808 - 1"),
                ("int mul", "*", 26, "4611686018427387904 * 2"),
                ("int neg", "-", 31, "-(-9223372036854775808)"),
                ("int div by zero", "/", 36, "1 / 0"),
                ("int div overflow", "/", 41, "-9223372036854775808 / -1"),
                ("int rem by zero", "%", 46, "1 % 0"),
                ("int rem overflow", "%", 51, "-9223372036854775808 % -1"),
                ("byte add", "+", 56, "255 + 1"),
                ("byte sub", "-", 61, "0 - 1"),
                ("byte mul", "*", 66, "16 * 16"),
                ("byte div by zero", "/", 71, "1 / 0"),
                ("byte rem by zero", "%", 76, "1 % 0"),
            ]
            .map(|(test, op, line, detail)| {
                format!(
                    "---- fixtures.overflow/{test} stderr ----\nerror: {op} failed at {file}:{line}:11: {detail}\n"
                )
            });
            for line in expected.iter().map(String::as_str).chain([
                "test result: FAILED. 1 passed; 13 failed; 0 filtered out",
                "test fixtures.overflow/in range passes ... ok",
            ]) {
                assert!(
                    run.stdout.contains(line),
                    "{label}: no `{line}` in\n{}",
                    run.stdout
                );
            }
            reports
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push((label, without_time(&run.stdout)));
        }
    });
    let reports = reports.into_inner().unwrap_or_else(|e| e.into_inner());
    assert_eq!(reports.len(), 4);
    for (label, report) in &reports[1..] {
        assert_eq!(
            report, &reports[0].1,
            "{label} differs from {}",
            reports[0].0
        );
    }
}
