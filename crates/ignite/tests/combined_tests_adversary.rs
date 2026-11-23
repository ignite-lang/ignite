//! Adversary (story `combined-test-runs-keep-every-sibling-suite`):
//! `ignite test` over several roots whose sibling modules share module
//! paths (`docs/slice.md` 12.7 and 25.6). Every layout is written under the
//! target's temporary directory, so the differential corpus never sees it.
//! The grouping does not depend on the target, so every case but the
//! `--target all` one runs on LLVM only, which needs no cargo build.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::common;

fn root() -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join("combined-tests-adversary")
}

/// Write `files` (relative path, text) under `root()/case` and return that
/// directory.
fn layout(case: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = root().join(case);
    for (name, text) in files {
        let path = dir.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        if std::fs::read_to_string(&path).ok().as_deref() != Some(*text) {
            std::fs::write(&path, text).unwrap();
        }
    }
    dir
}

/// A symbolic link at `link` to `target`, made once.
#[cfg(unix)]
fn symlink(target: &str, link: &Path) {
    if std::fs::symlink_metadata(link).is_err() {
        std::os::unix::fs::symlink(target, link).unwrap();
    }
}

struct Run {
    status: i32,
    stdout: String,
    stderr: String,
}

/// `ignite test <args> --target <target>` with `cwd` as working directory,
/// into an output directory of `case` and `target`.
fn ignite_test(case: &str, target: &str, cwd: &Path, args: &[&str]) -> Run {
    let out = root().join("out").join(case).join(target);
    let mut command = Command::new(env!("CARGO_BIN_EXE_ignite"));
    command
        .current_dir(cwd)
        .arg("test")
        .args(args)
        .args(["--target", target, "--out"])
        .arg(&out)
        .arg("--runtime")
        .arg(common::runner_archive());
    if target != "llvm" {
        command.arg("--target-dir").arg(out.join("cargo"));
    }
    let output = command.output().expect("run ignite test");
    Run {
        status: output.status.code().expect("ignite test exits"),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// The result lines of the tests a report ran, sorted.
fn ran(report: &str) -> Vec<&str> {
    let mut tests: Vec<&str> = report
        .lines()
        .filter_map(|line| line.strip_prefix("test "))
        .filter(|line| !line.starts_with("result: "))
        .collect();
    tests.sort_unstable();
    tests
}

fn executables(report: &str) -> usize {
    report.matches("test result: ").count()
}

/// A root of package `p.<name>` importing `pick` from the module `module`.
fn root_file(name: &str, module: &str, test: &str) -> String {
    format!("package p.{name}\nimport {module}.pick\ntest \"{test}\" {{ assert pick() == 1 }}\n")
}

/// A module `p.<name>` with `pick` and one test that passes or fails.
fn picked(name: &str, test: &str, passes: bool) -> String {
    format!("package p.{name}\nfn pick() -> Int = 1\ntest \"{test}\" {{ assert {passes} }}\n")
}

/// Story acceptance control and `docs/slice.md` 12.7: "two roots in one
/// directory that share one physical shared.ignite still run its tests
/// once"; "programs sharing one sibling file share an executable and run
/// its tests once". Two program files of one package in one directory
/// (25.6 names that layout: `fixtures/codespec_siblings` has `app.ignite`
/// and `reader.ignite`, both `package cs`) share the sibling `q.s`. Their
/// one module path puts them in two executables, each of which runs `q.s`'s
/// tests, so its test is reported twice.
#[test]
fn one_package_roots_run_their_shared_sibling_once() {
    let dir = layout(
        "one-package",
        &[
            (
                "x.ignite",
                "package q\nimport q.s.f\ntest \"x\" { assert f() == 1 }\n",
            ),
            (
                "y.ignite",
                "package q\nimport q.s.f\ntest \"y\" { assert f() == 1 }\n",
            ),
            (
                "s.ignite",
                "package q.s\nfn f() -> Int = 1\ntest \"s\" { assert true }\n",
            ),
        ],
    );
    let run = ignite_test("one-package", "llvm", &dir, &["x.ignite", "y.ignite"]);
    assert_eq!(
        ran(&run.stdout),
        ["q.s/s ... ok", "q/x ... ok", "q/y ... ok"],
        "{}\n{}",
        run.stdout,
        run.stderr
    );
    assert_eq!(run.status, 0, "{}", run.stdout);
}

/// A chain of conflicts: `one` and `two` give `p.s` two files, `two` and
/// `three` give `p.t` two files, `one` and `three` agree. In every order
/// two executables suffice (the first group takes the third root back), and
/// every copy's tests run.
#[test]
fn a_chain_of_conflicts_needs_two_executables_in_every_order() {
    let a_one = root_file("one", "p.s", "first root");
    let a_s = picked("s", "s first", true);
    let b_two = "package p.two\nimport p.s.pick\nimport p.t.take\n\
                 test \"second root\" { assert pick() + take() == 2 }\n";
    let b_s = picked("s", "s second", false);
    let b_t = "package p.t\nfn take() -> Int = 1\ntest \"t second\" { assert true }\n";
    let c_three = "package p.three\nimport p.t.take\ntest \"third root\" { assert take() == 1 }\n";
    let c_t = "package p.t\nfn take() -> Int = 1\ntest \"t third\" { assert true }\n";
    let dir = layout(
        "chain",
        &[
            ("a/one.ignite", &a_one),
            ("a/s.ignite", &a_s),
            ("b/two.ignite", b_two),
            ("b/s.ignite", &b_s),
            ("b/t.ignite", b_t),
            ("c/three.ignite", c_three),
            ("c/t.ignite", c_t),
        ],
    );
    let (one, two, three) = ("a/one.ignite", "b/two.ignite", "c/three.ignite");
    for args in [[one, two, three], [two, one, three], [three, two, one]] {
        let run = ignite_test("chain", "llvm", &dir, &args);
        assert_eq!(
            ran(&run.stdout),
            [
                "p.one/first root ... ok",
                "p.s/s first ... ok",
                "p.s/s second ... FAILED",
                "p.t/t second ... ok",
                "p.t/t third ... ok",
                "p.three/third root ... ok",
                "p.two/second root ... ok",
            ],
            "{args:?}: {}\n{}",
            run.stdout,
            run.stderr
        );
        assert_eq!(run.status, 1, "{args:?}: {}", run.stdout);
        assert_eq!(executables(&run.stdout), 2, "{args:?}: {}", run.stdout);
    }
}

/// The conflict is only transitive: each root imports `p.x`, one physical
/// file (`b/x.ignite` links to `a/x.ignite`), which imports `p.shared` from
/// the root's directory, where the two copies differ.
#[cfg(unix)]
#[test]
fn a_transitive_sibling_from_another_file_splits_the_run() {
    let via = |name: &str, test: &str| {
        format!("package p.{name}\nimport p.x.get\ntest \"{test}\" {{ assert get() == 1 }}\n")
    };
    let dir = layout(
        "transitive",
        &[
            ("a/one.ignite", &via("one", "first root")),
            (
                "a/x.ignite",
                "package p.x\nimport p.shared.pick\nfn get() -> Int = pick()\n",
            ),
            ("a/shared.ignite", &picked("shared", "shared first", true)),
            ("b/two.ignite", &via("two", "second root")),
            ("b/shared.ignite", &picked("shared", "shared second", false)),
        ],
    );
    symlink("../a/x.ignite", &dir.join("b/x.ignite"));
    for args in [
        ["a/one.ignite", "b/two.ignite"],
        ["b/two.ignite", "a/one.ignite"],
    ] {
        let run = ignite_test("transitive", "llvm", &dir, &args);
        assert_eq!(
            ran(&run.stdout),
            [
                "p.one/first root ... ok",
                "p.shared/shared first ... ok",
                "p.shared/shared second ... FAILED",
                "p.two/second root ... ok",
            ],
            "{args:?}: {}\n{}",
            run.stdout,
            run.stderr
        );
        assert_eq!(run.status, 1, "{args:?}: {}", run.stdout);
    }
}

/// One physical sibling reached by several spellings: a relative path with
/// `./`, one through `..`, an absolute one through a symbolic link to the
/// directory, and the shared module itself named as a root. One
/// executable, `p.shared`'s tests once: the grouping compares canonical
/// files, not spellings.
#[cfg(unix)]
#[test]
fn spellings_of_one_sibling_file_share_one_executable() {
    let dir = layout(
        "spellings",
        &[
            ("a/one.ignite", &root_file("one", "p.shared", "first root")),
            (
                "a/three.ignite",
                &root_file("three", "p.shared", "third root"),
            ),
            ("a/shared.ignite", &picked("shared", "shared first", true)),
        ],
    );
    symlink("a", &dir.join("link"));
    let three = dir.join("link/three.ignite").display().to_string();
    let run = ignite_test(
        "spellings",
        "llvm",
        &dir,
        &["./a/one.ignite", "a/../a/shared.ignite", &three],
    );
    assert_eq!(
        ran(&run.stdout),
        [
            "p.one/first root ... ok",
            "p.shared/shared first ... ok",
            "p.three/third root ... ok",
        ],
        "{}\n{}",
        run.stdout,
        run.stderr
    );
    assert_eq!(run.status, 0, "{}", run.stdout);
    assert_eq!(executables(&run.stdout), 1, "{}", run.stdout);
}

/// The reproducer's four files.
fn reproducer(case: &str) -> PathBuf {
    layout(
        case,
        &[
            ("a/one.ignite", &root_file("one", "p.shared", "first root")),
            ("a/shared.ignite", &picked("shared", "shared first", true)),
            ("b/two.ignite", &root_file("two", "p.shared", "second root")),
            ("b/shared.ignite", &picked("shared", "shared second", false)),
        ],
    )
}

/// A FILTER that selects nothing in the second executable: that executable
/// runs no test and exits 0, so the run passes; a FILTER that selects the
/// failing copy fails it, whichever executable holds it.
#[test]
fn a_filter_across_split_executables() {
    let dir = reproducer("filter");
    let args = ["b/two.ignite", "a/one.ignite"];
    let first = ignite_test("filter", "llvm", &dir, &[args[0], args[1], "first"]);
    assert_eq!(
        ran(&first.stdout),
        ["p.one/first root ... ok", "p.shared/shared first ... ok"],
        "{}\n{}",
        first.stdout,
        first.stderr
    );
    assert_eq!(first.status, 0, "{}", first.stdout);
    assert_eq!(executables(&first.stdout), 2, "{}", first.stdout);
    let shared = ignite_test("filter", "llvm", &dir, &[args[0], args[1], "shared"]);
    assert_eq!(
        ran(&shared.stdout),
        [
            "p.shared/shared first ... ok",
            "p.shared/shared second ... FAILED"
        ],
        "{}\n{}",
        shared.stdout,
        shared.stderr
    );
    assert_eq!(shared.status, 1, "{}", shared.stdout);
}

/// Story outcome: "roots whose transitive modules share a dotted name but
/// come from different source files run in separate test executables".
/// `one.ignite` reaches `std.option` from `std/option.ignite`; the root
/// `option.ignite` beside it is the module `std.option` from a local file
/// (as `ignite rfc import` writes `package std.rfc.rfc<number>` by
/// default, `docs/slice.md` 19.8). Each alone runs its one test and exits
/// 0; together the LLVM test build panics (`call of Fn(QualName { module:
/// "std.option", name: "unwrap_or" })`) and the run exits 101.
#[test]
#[ignore = "story:local-module-with-a-std-path-in-a-combined-test-run"]
fn a_local_module_with_a_standard_library_path_keeps_both_suites() {
    let dir = layout(
        "std-path",
        &[
            (
                "option.ignite",
                "package std.option\nfn pick() -> Int = 1\ntest \"local option\" { assert pick() == 1 }\n",
            ),
            (
                "one.ignite",
                "package p.one\nimport std.option.{unwrap_or}\n\
                 test \"uses std\" { assert unwrap_or(Some(1), 0) == 1 }\n",
            ),
        ],
    );
    for args in [
        ["option.ignite", "one.ignite"],
        ["one.ignite", "option.ignite"],
    ] {
        let run = ignite_test("std-path", "llvm", &dir, &args);
        assert_eq!(
            ran(&run.stdout),
            ["p.one/uses std ... ok", "std.option/local option ... ok"],
            "{args:?}: {}\n{}",
            run.stdout,
            run.stderr
        );
        assert_eq!(run.status, 0, "{args:?}: {}", run.stderr);
    }
}

/// `--target all` runs both executables on each target: every copy's tests
/// twice, four reports, exit 1.
#[test]
fn target_all_runs_every_sibling_suite_on_each_target() {
    let dir = reproducer("all");
    let run = ignite_test("all", "all", &dir, &["a/one.ignite", "b/two.ignite"]);
    assert_eq!(
        ran(&run.stdout),
        [
            "p.one/first root ... ok",
            "p.one/first root ... ok",
            "p.shared/shared first ... ok",
            "p.shared/shared first ... ok",
            "p.shared/shared second ... FAILED",
            "p.shared/shared second ... FAILED",
            "p.two/second root ... ok",
            "p.two/second root ... ok",
        ],
        "{}\n{}",
        run.stdout,
        run.stderr
    );
    assert_eq!(run.status, 1, "{}", run.stdout);
    assert_eq!(executables(&run.stdout), 4, "{}", run.stdout);
}
