//! `ignite test` end to end on both targets (`docs/slice.md` section 12):
//! the fixtures in `tests/fixtures/tests` are built into test executables
//! and run; the report on stdout and the exit status are checked.
//!
//! Every test runs its two targets on two threads ([`on_both_targets`]).
//! The invocations of one target form two chains ([`Chain`]), each with
//! an output directory, a cargo target directory and a lock of its own,
//! so a Rust build (cargo) and an LLVM build (clang) overlap and so do the
//! chains. The fixtures chain builds `slow`, `basics`, `targets`, `builtins`,
//! `store` and `fields` into one executable once per target ([`fixtures_run`]) and
//! runs the filter, list and overflow invocations over the same files,
//! which cargo keeps incremental. The groups chain holds the invocations
//! that change the project's shape (a second group `tests-2`, a standard
//! library of its own): each claims the cargo target directory and
//! rebuilds everything in it (`claim_target_dir`), which would cost the
//! fixtures chain a rebuild of `ignite-std` per switch. `ignite build` of
//! the prober has a cargo target directory of its own to avoid rebuilding
//! the fixtures when their generated projects alternate.

use crate::common;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

const TARGETS: [&str; 2] = ["rust", "llvm"];

/// The fixture programs of one test executable, in command-line order:
/// `slow` before `basics` shows the order is the command line's, not the
/// alphabet's.
const FIXTURES: [&str; 7] = [
    "slow.ignite",
    "basics.ignite",
    "targets.ignite",
    "builtins.ignite",
    "store.ignite",
    "fields.ignite",
    "clones.ignite",
];

/// The invocations of one target that share an output directory and a
/// cargo target directory; one of each runs at a time.
#[derive(Clone, Copy)]
enum Chain {
    Fixtures = 0,
    Groups = 1,
}

static LOCKS: [[Mutex<()>; 2]; 2] = [
    [Mutex::new(()), Mutex::new(())],
    [Mutex::new(()), Mutex::new(())],
];

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

fn chain_dir(chain: Chain) -> PathBuf {
    out_dir().join(match chain {
        Chain::Fixtures => "fixtures",
        Chain::Groups => "groups",
    })
}

struct Run {
    status: i32,
    stdout: String,
    stderr: String,
}

/// `ignite test <args> --target <target>` in `chain`'s directories.
fn ignite_test(chain: Chain, target: &str, args: &[&str]) -> Run {
    let _guard = LOCKS[chain as usize][target_index(target)]
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let dir = chain_dir(chain);
    let mut command = Command::new(env!("CARGO_BIN_EXE_ignite"));
    command
        .arg("test")
        .args(args)
        .args(["--target", target, "--out"])
        .arg(dir.join(target))
        .arg("--runtime")
        .arg(common::runner_archive());
    if target == "rust" {
        command.arg("--target-dir").arg(dir.join("cargo"));
    }
    let output = command.output().expect("run ignite test");
    Run {
        status: output.status.code().expect("ignite test exits"),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// `ignite test` of the [`FIXTURES`] with `args` after them.
fn fixtures_test(target: &str, args: &[&str]) -> Run {
    let files: Vec<String> = FIXTURES.iter().map(|name| fixture(name)).collect();
    let mut all: Vec<&str> = files.iter().map(String::as_str).collect();
    all.extend_from_slice(args);
    ignite_test(Chain::Fixtures, target, &all)
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

/// The run of the [`FIXTURES`] in one executable with `--timeout 1` on
/// `target`, once per test binary: ten tests read it. `slow`'s `never
/// ends` is the test the timeout kills; the other 28 finish well inside
/// a second.
fn fixtures_run(target: &str) -> &'static Run {
    static RUNS: [OnceLock<Run>; 2] = [OnceLock::new(), OnceLock::new()];
    RUNS[target_index(target)].get_or_init(|| fixtures_test(target, &["--timeout", "1"]))
}

#[test]
fn counts_results_and_failures() {
    on_both_targets(|target| {
        let run = fixtures_run(target);
        let out = &run.stdout;
        assert_eq!(run.status, 1, "{target}: {out}\n{}", run.stderr);
        assert!(out.starts_with("running 32 tests\n"), "{target}: {out}");
        for line in [
            "test fixtures.basics/arithmetic passes ... ok",
            "test fixtures.basics/int differs ... FAILED",
            "test fixtures.basics/exit code 3 ... FAILED (exited with code 3)",
            "test fixtures.basics/index out of range panics ... FAILED",
            "test fixtures.basics/output hidden on success ... ok",
            "test fixtures.basics/tcp echo through a task ... ok",
            "test result: FAILED. 21 passed; 11 failed; 0 filtered out; finished in ",
        ] {
            assert!(out.contains(line), "{target}: no `{line}` in\n{out}");
        }
        assert!(
            out.contains(
                "failures:\n    fixtures.slow/never ends\n    fixtures.basics/int differs\n"
            ),
            "{target}: {out}"
        );
    });
}

#[test]
fn assert_and_try_reports() {
    let path = fixture("basics.ignite");
    on_both_targets(|target| {
        let out = &fixtures_run(target).stdout;
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
        let out = &fixtures_run(target).stdout;
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
            without_time(&fixtures_run(target).stdout)
                .lines()
                .filter(|line| !line.starts_with("error: panic at "))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect();
    assert_eq!(reports[0], reports[1]);
}

/// A substring filter runs the matching tests and counts the rest as
/// filtered out, `--exact` matches one name, and a filter nothing matches
/// runs no test and exits 0.
#[test]
fn filter_exact_passing_and_none() {
    on_both_targets(|target| {
        let exact = fixtures_test(target, &["fixtures.basics/int differs", "--exact"]);
        assert_eq!(exact.status, 1, "{target}: {}", exact.stdout);
        assert!(exact.stdout.starts_with("running 1 test\n"), "{target}");
        assert!(
            exact
                .stdout
                .contains("test result: FAILED. 0 passed; 1 failed; 31 filtered out;"),
            "{target}: {}",
            exact.stdout
        );
        let passing = fixtures_test(target, &["passes"]);
        assert_eq!(passing.status, 0, "{target}: {}", passing.stdout);
        assert_eq!(
            without_time(&passing.stdout),
            "running 1 test\ntest fixtures.basics/arithmetic passes ... ok\n\ntest result: ok. 1 passed; 0 failed; 31 filtered out",
            "{target}"
        );
        let none = fixtures_test(target, &["no such test"]);
        assert_eq!(none.status, 0, "{target}");
        assert_eq!(
            without_time(&none.stdout),
            "running 0 tests\n\ntest result: ok. 0 passed; 0 failed; 32 filtered out",
            "{target}"
        );
    });
}

/// Programs with different packages share one test executable: one list,
/// one count, the tests in command-line order of their files (`slow`
/// before `basics`).
#[test]
fn files_share_one_executable() {
    on_both_targets(|target| {
        let list = fixtures_test(target, &["fa", "--list"]);
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
            let run = ignite_test(Chain::Groups, target, &[&a, &b]);
            assert_eq!(run.status, 0, "{target}: {}\n{}", run.stdout, run.stderr);
            assert_eq!(
                without_time(&run.stdout),
                "running 1 test\ntest main/first file ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 filtered out\nrunning 2 tests\ntest main/second file ... ok\ntest main/second file again ... ok\n\ntest result: ok. 2 passed; 0 failed; 0 filtered out",
                "{target}"
            );
        }
    });
}

/// `--timeout 1` kills `slow`'s `never ends` after a second and reports
/// it; the run's other tests are untouched ([`fixtures_run`]).
#[test]
fn timeout_kills_a_test() {
    on_both_targets(|target| {
        let out = &fixtures_run(target).stdout;
        assert!(
            out.contains("test fixtures.slow/never ends ... FAILED (timed out after 1 s)\n"),
            "{target}: {out}"
        );
        assert!(
            out.contains("test fixtures.slow/fast ... ok\n"),
            "{target}: {out}"
        );
    });
}

#[test]
fn compile_error_exits_2() {
    on_both_targets(|target| {
        let run = ignite_test(Chain::Groups, target, &[&fixture("broken.ignite")]);
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
        let run = ignite_test(
            Chain::Groups,
            target,
            &[&fixture("std/greet.ignite"), "--std", &std],
        );
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
        let out = &fixtures_run(target).stdout;
        for row in ["32", "33", "34", "35"] {
            let prefix = format!("test fixtures.targets/row {row}: ");
            let line = out
                .lines()
                .find(|line| line.starts_with(&prefix))
                .unwrap_or_else(|| panic!("{target}: no row {row} in\n{out}"));
            assert!(line.ends_with(" ... ok"), "{target}: {line}");
        }
    });
}

/// The substrate builtin `Clock.monotonic` of slice 12 (`docs/slice.md`
/// 16.13) under `ignite test` on both targets (`fixtures/tests/builtins.ignite`).
#[test]
fn substrate_builtins_run_on_both_targets() {
    on_both_targets(|target| {
        let out = &fixtures_run(target).stdout;
        assert!(
            out.contains("test tests.builtins/Clock.monotonic: two reads are ordered ... ok\n"),
            "{target}: {out}"
        );
    });
}

/// A failure inside `Store.update`'s `f`, which runs under the store's
/// lock, ends its task with its report and leaves the lock free on both
/// targets (`fixtures/tests/store.ignite`). On LLVM the failure used to
/// `longjmp` past the runtime frame holding the lock, so the next use of
/// the store waited forever.
#[test]
fn a_failing_store_update_ends_its_task_and_frees_the_store() {
    let file = fixture("store.ignite");
    on_both_targets(|target| {
        let out = &fixtures_run(target).stdout;
        for line in [
            "test fixtures.store/an overflow in update in a task leaves the store usable ... ok\n",
            "test fixtures.store/an overflow in update ends its test ... FAILED\n",
            &format!(
                "---- fixtures.store/an overflow in update ends its test stderr ----\nerror: + failed at {file}:44:23: 9223372036854775807 + 1\n"
            ),
        ] {
            assert!(out.contains(line), "{target}: no `{line}` in\n{out}");
        }
    });
}

/// `Store.upsert` on both targets (`fixtures/tests/store.ignite`): it
/// inserts on an empty id and replaces a held value, 8 tasks that each
/// upsert one new id 500 times leave 4,000 (`update` then `put` lost
/// writes there, `story:store-atomic-upsert`), and an overflow inside its
/// `f` ends the task and leaves the store usable.
#[test]
fn store_upsert_is_atomic_on_both_targets() {
    on_both_targets(|target| {
        let out = &fixtures_run(target).stdout;
        for name in [
            "upsert inserts on an empty id and replaces a held value",
            "upsert from 8 tasks on one new id keeps every write",
            "an overflow in upsert in a task leaves the store usable",
        ] {
            let line = format!("test fixtures.store/{name} ... ok\n");
            assert!(out.contains(&line), "{target}: no `{line}` in\n{out}");
        }
    });
}

/// Two refusals the checker made from declared types alone, on both
/// targets (`fixtures/tests/checker_names.ignite`, pusher gaps G14 and
/// G15): a user fn named like a builtin `List` method is called by a plain
/// call while method syntax keeps the builtin, and a `Store` field of an
/// unannotated record-literal binding is changed and read.
#[test]
fn builtin_named_fns_and_record_literal_receivers_run_on_both_targets() {
    let file = fixture("checker_names.ignite");
    on_both_targets(|target| {
        let run = ignite_test(Chain::Groups, target, &[&file]);
        assert_eq!(run.status, 0, "{target}: {}\n{}", run.stdout, run.stderr);
        assert!(
            run.stdout.contains("test result: ok. 3 passed; 0 failed;"),
            "{target}: {}",
            run.stdout
        );
    });
}

/// `Store.values` while tasks update two ids of one shard, on both targets
/// (`fixtures/tests/store_load.ignite`, story `store-adversary-findings`):
/// every list holds every id, no value goes back, no list waits 500 ms
/// for the writers, and no write is lost.
#[test]
#[ignore = "story:llvm-store-update-off-lock: red on LLVM under load, base and head"]
fn store_values_under_updates_of_one_shard_on_both_targets() {
    let file = fixture("store_load.ignite");
    on_both_targets(|target| {
        let run = ignite_test(Chain::Groups, target, &[&file]);
        assert_eq!(run.status, 0, "{target}: {}\n{}", run.stdout, run.stderr);
        assert!(
            run.stdout.contains("test result: ok. 1 passed; 0 failed;"),
            "{target}: {}",
            run.stdout
        );
    });
}

/// Fields of list elements on both targets (`fixtures/tests/fields.ignite`):
/// read, passed by reference and compared (the Rust target borrows the
/// field from the list, `ignite-rustgen` `element_field`), nested, read from
/// a `Store`, read before the element or its list changes, and matched.
#[test]
fn fields_of_elements_on_both_targets() {
    on_both_targets(|target| {
        let out = &fixtures_run(target).stdout;
        for name in [
            "a field of an element is read, passed by reference and compared",
            "nested fields of elements",
            "a field of a Store read",
            "the element changes after its field is read",
            "a match on a field of an element",
        ] {
            let line = format!("test fixtures.fields/{name} ... ok\n");
            assert!(out.contains(&line), "{target}: no `{line}` in\n{out}");
        }
    });
}

/// The copies the Rust target leaves out (`fixtures/tests/clones.ignite`,
/// story:rustgen-redundant-clones) keep the values the same on both
/// targets: a byte string literal read by reference and by value, and a
/// `Store.update` or `Store.upsert` whose result is discarded or read for
/// one field.
#[test]
fn redundant_clones_on_both_targets() {
    on_both_targets(|target| {
        let out = &fixtures_run(target).stdout;
        for name in [
            "a byte string literal read by reference and by value",
            "a discarded update or upsert stores the new value",
            "a field of an upsert result is the stored field",
        ] {
            let line = format!("test fixtures.clones/{name} ... ok\n");
            assert!(out.contains(&line), "{target}: no `{line}` in\n{out}");
        }
    });
}

/// `Endpoint.of` on both targets (`docs/slice.md` 16.13, 16.9): the
/// `prober` of `fixtures/tests/builtins.ignite` fails without a `--ref`
/// binding and prints the endpoint of one.
#[test]
fn endpoint_of_reads_the_ref_binding_on_both_targets() {
    let source = fixture("builtins.ignite");
    on_both_targets(|target| {
        let dir = out_dir().join("prober");
        let mut build = Command::new(env!("CARGO_BIN_EXE_ignite"));
        build
            .arg("build")
            .arg(&source)
            .args(["--target", target, "--out"])
            .arg(dir.join(target));
        if target == "llvm" {
            build.arg("--runtime").arg(common::runtime_archive());
        } else {
            build.arg("--target-dir").arg(dir.join("cargo"));
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
    });
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
            let run = ignite_test(Chain::Fixtures, target, &args);
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

/// `Text` and `Bytes` equality against literals (`match` arms, constant
/// patterns, `==` and `!=`), which the LLVM target lowers inline: empty
/// values, literals shorter and longer than the value, equal lengths with
/// other bytes, non-ASCII text and nested patterns pass on both targets in
/// dev and in release (`fixtures/tests/text_eq.ignite`).
#[test]
fn text_equality_against_literals() {
    let file = fixture("text_eq.ignite");
    on_both_targets(|target| {
        for profile in [&[][..], &["--release"][..]] {
            let mut args = vec![file.as_str()];
            args.extend(profile.iter().copied());
            let run = ignite_test(Chain::Fixtures, target, &args);
            assert_eq!(
                run.status, 0,
                "{target} {profile:?}: {}\n{}",
                run.stdout, run.stderr
            );
            assert!(
                run.stdout
                    .contains("test result: ok. 8 passed; 0 failed; 0 filtered out"),
                "{target} {profile:?}: {}",
                run.stdout
            );
        }
    });
}

/// The language gaps of story codespec-friction (`docs/slice.md` 25):
/// `<`, `<=`, `>`, `>=` on `Text` in byte order, the order of a `Map`'s
/// `Text` keys, and `wrapping_add`, `wrapping_sub`, `wrapping_mul` on
/// `Int`, the same on both targets in dev and in release
/// (`fixtures/tests/codespec.ignite`).
#[test]
fn codespec_friction_on_both_targets() {
    let file = fixture("codespec.ignite");
    on_both_targets(|target| {
        for profile in [&[][..], &["--release"][..]] {
            let mut args = vec![file.as_str()];
            args.extend(profile.iter().copied());
            let run = ignite_test(Chain::Fixtures, target, &args);
            assert_eq!(
                run.status, 0,
                "{target} {profile:?}: {}\n{}",
                run.stdout, run.stderr
            );
            assert!(
                run.stdout
                    .contains("test result: ok. 6 passed; 0 failed; 0 filtered out"),
                "{target} {profile:?}: {}",
                run.stdout
            );
        }
    });
}

/// `checked_add`, `checked_sub` and `checked_mul` on `Int` (`docs/slice.md`
/// 25.2): `None` on overflow at both bounds of `Int`, `Some` otherwise,
/// the same on both targets in dev and in release
/// (`fixtures/tests/checked.ignite`).
#[test]
fn checked_arithmetic_on_both_targets() {
    let file = fixture("checked.ignite");
    on_both_targets(|target| {
        for profile in [&[][..], &["--release"][..]] {
            let mut args = vec![file.as_str()];
            args.extend(profile.iter().copied());
            let run = ignite_test(Chain::Fixtures, target, &args);
            assert_eq!(
                run.status, 0,
                "{target} {profile:?}: {}\n{}",
                run.stdout, run.stderr
            );
            assert!(
                run.stdout
                    .contains("test result: ok. 4 passed; 0 failed; 0 filtered out"),
                "{target} {profile:?}: {}",
                run.stdout
            );
        }
    });
}

/// An in-place builtin changes the variable only through a place rooted at
/// it, and a copy through `at` or `?`, the same on both targets
/// (`fixtures/tests/in_place.ignite`, `ignite_types::ownership`). Before
/// story:shared-ownership-analysis the LLVM target changed the variable
/// through `at` and `?` (its `region::root` followed them) and the Rust
/// target a copy.
#[test]
fn in_place_changes_on_both_targets() {
    let file = fixture("in_place.ignite");
    on_both_targets(|target| {
        let run = ignite_test(Chain::Fixtures, target, &[file.as_str()]);
        assert_eq!(run.status, 0, "{target}: {}\n{}", run.stdout, run.stderr);
        assert!(
            run.stdout
                .contains("test result: ok. 5 passed; 0 failed; 0 filtered out"),
            "{target}: {}",
            run.stdout
        );
    });
}

/// List and `Bytes` element assignment, `List + List` and `Stderr` (`docs/slice.md`
/// 25.3 to 25.5) on both targets in dev and in release
/// (`fixtures/tests/codespec_lists.ignite`): an assignment out of range
/// fails with the report of an index out of range, and what a test wrote to
/// `Stdout` and to `Stderr` is shown under its own stream.
#[test]
fn codespec_stderr_and_lists_on_both_targets() {
    let file = fixture("codespec_lists.ignite");
    on_both_targets(|target| {
        for profile in [&[][..], &["--release"][..]] {
            let mut args = vec![file.as_str()];
            args.extend(profile.iter().copied());
            let run = ignite_test(Chain::Fixtures, target, &args);
            let label = format!("{target} {profile:?}");
            assert_eq!(run.status, 1, "{label}: {}\n{}", run.stdout, run.stderr);
            let name = "fixtures.codespec_lists";
            for line in [
                "test result: FAILED. 5 passed; 3 failed; 0 filtered out".to_string(),
                format!(
                    "---- {name}/element assignment out of range fails as indexing does stderr ----\nerror: index failed at {file}:52:3: index 3 out of range for length 3\n"
                ),
                format!(
                    "---- {name}/Bytes element assignment out of range fails as indexing does stderr ----\nerror: index failed at {file}:98:3: index 2 out of range for length 2\n"
                ),
                format!("---- {name}/Stderr is a stream of its own stdout ----\nto out\n"),
                format!("---- {name}/Stderr is a stream of its own stderr ----\nto err\n"),
            ] {
                assert!(
                    run.stdout.contains(&line),
                    "{label}: no `{line}` in\n{}",
                    run.stdout
                );
            }
        }
    });
}

/// Both native targets use one preparation while retaining their normal
/// runner selection, reports and failure status. A Rust failure must not
/// prevent the LLVM suite from executing.
#[test]
fn combined_targets_keep_runner_arguments_and_continue_after_failure() {
    let dir = out_dir().join("combined");
    let source = fixture("basics.ignite");
    let run = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_ignite"))
            .arg("test")
            .arg(&source)
            .args(["--target=all", "--out"])
            .arg(&dir)
            .arg("--target-dir")
            .arg(dir.join("cargo"))
            .arg("--runtime")
            .arg(common::runner_archive())
            .args(args)
            .output()
            .expect("run combined native tests");
        Run {
            status: output.status.code().unwrap(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    };
    let selected = "fixtures.basics/arithmetic passes";
    let listed = run(&[
        selected,
        "--exact",
        "--list",
        "--jobs",
        "1",
        "--timeout",
        "1",
    ]);
    assert_eq!(listed.status, 0, "{}", listed.stderr);
    assert_eq!(
        listed.stdout,
        format!("{selected}: test\n1 test\n").repeat(2)
    );
    let passed = run(&[selected, "--exact", "--jobs", "1", "--timeout", "1"]);
    assert_eq!(passed.status, 0, "{}", passed.stderr);
    assert_eq!(
        passed.stdout.matches("test result: ok. 1 passed;").count(),
        2,
        "{}",
        passed.stdout
    );
    let failed = run(&[
        "fixtures.basics/int differs",
        "--exact",
        "--jobs",
        "1",
        "--timeout",
        "1",
    ]);
    assert_eq!(failed.status, 1, "{}", failed.stderr);
    assert_eq!(
        failed
            .stdout
            .matches("test fixtures.basics/int differs ... FAILED")
            .count(),
        2,
        "{}",
        failed.stdout
    );
    assert!(dir.join("tests/rust").is_dir());
    assert!(dir.join("tests/llvm").is_dir());
}

#[test]
fn both_targets_capture_output_larger_than_pipe_buffers_without_deadlock() {
    let dir = out_dir().join("large-streams");
    let output = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("test")
        .arg(fixture("large_output.ignite"))
        .args(["--target", "all", "--timeout", "2", "--out"])
        .arg(&dir)
        .arg("--target-dir")
        .arg(dir.join("cargo"))
        .arg("--runtime")
        .arg(common::runner_archive())
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert_eq!(text.matches(&"0123456789abcdef".repeat(8192)).count(), 4);
    assert_eq!(
        text.matches("---- fixtures.large_output/large streams stdout ----")
            .count(),
        2
    );
    assert_eq!(
        text.matches("---- fixtures.large_output/large streams stderr ----")
            .count(),
        2
    );
}
