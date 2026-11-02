//! Slice 9 on the Rust target (`docs/slice.md` section 13): the inline
//! tests of `tests/slice9/*.ignite` are lowered as a test build, built with
//! cargo and run. `features.ignite` covers rows 4 to 17 and must pass;
//! `failures.ignite` must fail with both values shown as debug text.
//! Row 14: 100,000-term chains lower without overflow into Rust chains of
//! fewer than 64 operators; 10,000-term chains build and run (100,000 in
//! an ignored test).

use crate::common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use ignite_rustgen::{TEST_EXECUTABLE, write_project};

fn target_dir() -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| common::workspace().join("target"))
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/slice9")
        .join(name)
}

/// Lower `file` as a test build, build it and return a copy of its
/// `ignite-test` executable.
fn build_tests(file: &Path, name: &str) -> PathBuf {
    build_tests_with(file, name, &common::repository_std())
}

/// [`build_tests`] over the standard library at `std_root`.
fn build_tests_with(file: &Path, name: &str, std_root: &Path) -> PathBuf {
    let substrate = common::workspace().join("crates/ignite-io");
    let substrate = substrate.canonicalize().unwrap_or(substrate);
    let (project, std) = common::lower_tests(file, std_root, &substrate);
    let target = target_dir();
    let out = target.join("ignite-rustgen-slice9").join(name);
    write_project(&project, &out).unwrap();
    write_project(&std, &out.join("std")).unwrap();
    let builds = target
        .join("ignite-rustgen-slice9")
        .join(format!("{name}-target"));
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(cargo)
        .arg("build")
        .arg("--manifest-path")
        .arg(out.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(&builds)
        .env("CARGO_INCREMENTAL", "0")
        .status()
        .unwrap();
    assert!(status.success(), "cargo build of {} failed", out.display());
    let file = format!("{TEST_EXECUTABLE}{}", std::env::consts::EXE_SUFFIX);
    builds.join("debug").join(file)
}

fn run(executable: &Path, args: &[&str]) -> Output {
    Command::new(executable).args(args).output().unwrap()
}

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn slice9_features_pass() {
    let executable = build_tests(&fixture("features.ignite"), "features");
    let output = run(&executable, &[]);
    assert!(output.status.success(), "{}", text(&output));
}

/// An assignment to a field boxes its value when the field's owner boxes
/// it (`docs/slice.md` section 13.4), also for an owner declared in another
/// module, and never for another record's field of the same name.
#[test]
fn assignments_box_by_the_fields_owner() {
    let executable = build_tests_with(&fixture("assign.ignite"), "assign", &fixture("std"));
    let output = run(&executable, &[]);
    assert!(output.status.success(), "{}", text(&output));
    assert!(text(&output).contains("1 passed"), "{}", text(&output));
}

#[test]
fn failed_asserts_show_debug_text() {
    let executable = build_tests(&fixture("failures.ignite"), "failures");
    let cases: &[(&str, &[&str])] = &[
        ("list", &["left: [1, 2]", "right: [1, 3]"]),
        (
            "record",
            &[
                "left: Todo { id: 1, title: \"milk\", done: false }",
                "right: Todo { id: 1, title: \"eggs\\n\", done: false }",
            ],
        ),
        ("option", &["left: None", "right: Some(3)"]),
        (
            "tree",
            &[
                "message: same trees",
                "left: Tree.Node(Tree.Leaf, 1)",
                "right: Tree.Node(Tree.Leaf, 1)",
            ],
        ),
        ("tuple", &["left: (1, [true])", "right: (1, [false])"]),
    ];
    for (name, expected) in cases {
        let output = run(&executable, &["--run-one", &format!("main/{name}")]);
        let shown = text(&output);
        assert_eq!(output.status.code(), Some(101), "{name}: {shown}");
        for line in *expected {
            assert!(shown.contains(line), "{name}: missing `{line}` in\n{shown}");
        }
    }
}

/// A program with `terms`-term chains of Int `+`, Text `+` and `&&`, a
/// left spine mixing `+` and `*` and a `&&` chain that must short-circuit,
/// and a test of their values (row 14).
fn chains_source(terms: usize) -> String {
    let mut source = String::from("fn ints() -> Int = 1");
    source.push_str(&" + 1".repeat(terms - 1));
    source.push_str("\nfn texts() -> Text = \"a\"");
    source.push_str(&" + \"a\"".repeat(terms - 1));
    source.push_str("\nfn both(n: Int) -> Bool = n > 0");
    source.push_str(&" && n > 0".repeat(terms - 1));
    source.push_str("\nfn mixed(n: Int) -> Int = ");
    source.push_str(&"(".repeat(100));
    source.push('n');
    source.push_str(&" + 1) * 0".repeat(100));
    source.push_str("\nfn guarded(xs: List<Int>) -> Bool = xs.len() > 0");
    source.push_str(&" && xs[0] > 0".repeat(199));
    source.push_str(&format!(
        "\ntest \"chains\" {{\n  assert ints() == {terms}\n  assert texts().len() == {terms}\n  assert both(1)\n  assert !both(0)\n  assert mixed(5) == 0\n  assert guarded([1])\n  assert !guarded([])\n}}\n"
    ));
    source
}

/// Write [`chains_source`] for `terms` as `<name>.ignite` in the shared
/// output directory.
fn chains_file(terms: usize, name: &str) -> PathBuf {
    let dir = target_dir().join("ignite-rustgen-slice9");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join(format!("{name}.ignite"));
    std::fs::write(&file, chains_source(terms)).unwrap();
    file
}

/// No generated line holds a Rust operator chain of 64 or more terms.
fn assert_chunked(generated: &str) {
    for line in generated.lines() {
        let operators = line.matches(" + ").count() + line.matches(" && ").count();
        assert!(
            operators < 64,
            "a chain of {operators} operators on one line"
        );
    }
}

fn build_and_run_chains(terms: usize, name: &str) {
    let executable = build_tests(&chains_file(terms, name), name);
    let dir = target_dir().join("ignite-rustgen-slice9").join(name);
    assert_chunked(&std::fs::read_to_string(dir.join("src/lib.rs")).unwrap());
    let output = run(&executable, &[]);
    assert!(output.status.success(), "{}", text(&output));
}

#[test]
fn long_chains_lower_without_overflow() {
    let file = chains_file(100_000, "chains-lower");
    let substrate = common::workspace().join("crates/ignite-io");
    let (project, _) = common::lower_tests(&file, &common::repository_std(), &substrate);
    assert_chunked(&project.lib_rs);
}

#[test]
fn long_chains_build_and_run() {
    build_and_run_chains(10_000, "chains");
}

/// The 100,000-term chains through rustc: minutes, not in the gate.
#[test]
#[ignore]
fn long_chains_build_and_run_100k() {
    build_and_run_chains(100_000, "chains-100k");
}
