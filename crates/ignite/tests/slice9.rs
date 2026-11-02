//! Slice 9 (`docs/slice.md` section 13) and the soundness probes of the
//! 2026-11-01 audit through `ignite test`: every fixture must pass on both
//! targets, failing asserts print the same debug text on both, and a
//! 10,000-term chain lowers, compiles and runs on the LLVM target (the
//! 100,000-term chain in the `#[ignore]` tests, run by hand).

use crate::common;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

/// One `ignite test` at a time: the Rust target shares cargo's target
/// directory between builds.
static BUILDS: Mutex<()> = Mutex::new(());

const TARGETS: [&str; 2] = ["rust", "llvm"];

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn out_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("ignite-slice9")
        .join(name)
}

struct Run {
    status: i32,
    stdout: String,
    stderr: String,
}

/// `ignite test <file> --target <target>`, each file in its own output
/// and cargo target directory, so one fixture's build never waits for or
/// invalidates another's.
fn ignite_test(target: &str, file: &Path, name: &str) -> Run {
    ignite_test_files(target, &[file.to_path_buf()], name)
}

/// `ignite test <files>… --target <target>`: one test executable per
/// target for every file given (`docs/slice.md` 12.7), so a group of
/// fixtures costs one build and one subprocess instead of one per file.
fn ignite_test_files(target: &str, files: &[PathBuf], name: &str) -> Run {
    let _guard = BUILDS.lock().unwrap_or_else(|e| e.into_inner());
    let output = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("test")
        .args(files)
        .args(["--target", target, "--out"])
        .arg(out_dir(name))
        .arg("--runtime")
        .arg(runner_runtime())
        .arg("--target-dir")
        .arg(out_dir(name).join("cargo"))
        .env("CARGO_INCREMENTAL", "0")
        .output()
        .expect("run ignite test");
    Run {
        status: output.status.code().expect("ignite test exits"),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn assert_passes(file: &Path, name: &str, targets: &[&str]) {
    assert_all_pass(&[file.to_path_buf()], name, targets);
}

/// Every file's tests pass on every target, the files built together.
fn assert_all_pass(files: &[PathBuf], name: &str, targets: &[&str]) {
    for target in targets {
        let run = ignite_test_files(target, files, name);
        let shown = files
            .iter()
            .map(|file| file.display().to_string())
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(
            run.status, 0,
            "{shown} on {target}:\n{}\n{}",
            run.stdout, run.stderr
        );
        assert!(run.stdout.contains("test result: ok."), "{}", run.stdout);
    }
}

#[test]
fn slice9_values_across_regions() {
    let file = workspace().join("crates/ignite/tests/fixtures/slice9/regions.ignite");
    assert_passes(&file, "regions", &TARGETS);
}

/// M1 to M3 of vision 10.1: a `var` bound to `x.to_bytes()`, a `for`
/// element, a match payload, a field or an element is a copy; pushing to it
/// leaves the source unchanged, once and over 20,000 loop releases.
#[test]
fn var_aliases_leave_the_source_unchanged() {
    let file = workspace().join("crates/ignite/tests/fixtures/audit/aliases.ignite");
    assert_passes(&file, "aliases", &TARGETS);
}

/// The probes of `dir` under `docs/audit/2026-11-01/`, as one list of files.
fn audit_probes(dir: &str, probes: &[&str]) -> Vec<PathBuf> {
    probes
        .iter()
        .map(|probe| {
            workspace()
                .join("docs/audit/2026-11-01")
                .join(dir)
                .join(format!("{probe}.ignite"))
        })
        .collect()
}

/// The probes of `docs/audit/2026-11-01/llvm/` that must compile and run
/// (the others are checker fixtures of E006). `range_exhaustive` drives
/// matches without `_` over their whole domain. One build per target for
/// all of them (slice 11; one per probe before took 3.8 s of the gate).
#[test]
fn audit_probes_pass() {
    let files = audit_probes(
        "llvm",
        &[
            "index_if",
            "index_if_only",
            "len_match",
            "later_arg",
            "sound_ok",
            "range_exhaustive",
        ],
    );
    assert_all_pass(&files, "audit-llvm", &TARGETS);
}

/// The probes of `docs/audit/2026-11-01/io/` that `ignite check` accepts
/// build and pass on both targets: `p1` (recursion through `Option` and an
/// enum payload, accepted since row 15), `p3c` (spawn of field arguments)
/// and the positive cases `p4` to `p10`. The others are checker fixtures.
/// One build per target for all of them (one per probe took 3.4 s).
#[test]
fn audit_io_probes_pass() {
    let files = audit_probes(
        "io",
        &[
            "p1_recursive",
            "p3c_spawn_fieldarg",
            "p4_guarded",
            "p5_params",
            "p6_if",
            "p9_generics",
            "p9b_generics",
            "p10_misc",
        ],
    );
    assert_all_pass(&files, "audit-io", &TARGETS);
}

/// The Rust target's slice-9 suite, run on the LLVM target.
#[test]
fn rust_target_features_pass_on_llvm() {
    let file = workspace().join("crates/ignite-rustgen/tests/slice9/features.ignite");
    assert_passes(&file, "features", &["llvm"]);
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

#[test]
fn failed_asserts_print_the_same_debug_text_on_both_targets() {
    let file = workspace().join("crates/ignite-rustgen/tests/slice9/failures.ignite");
    let reports: Vec<String> = TARGETS
        .iter()
        .map(|target| {
            let run = ignite_test(target, &file, "failures");
            assert_eq!(run.status, 1, "{target}:\n{}\n{}", run.stdout, run.stderr);
            without_time(&run.stdout)
        })
        .collect();
    assert!(
        reports[0].contains("right: Todo { id: 1, title: \"eggs\\n\", done: false }"),
        "{}",
        reports[0]
    );
    assert_eq!(reports[0], reports[1]);
}

/// Row 14: a program with left-deep chains of `terms` terms of `+` on
/// `Int` and `Text` and of `&&`, and a test of their values, written under
/// `name` (each test its own directory, since tests run in parallel).
fn chains(terms: usize, name: &str) -> PathBuf {
    let mut source = String::from("fn ints() -> Int = 1");
    source.push_str(&" + 1".repeat(terms - 1));
    source.push_str("\nfn texts() -> Text = \"a\"");
    source.push_str(&" + \"a\"".repeat(terms - 1));
    source.push_str("\nfn both(n: Int) -> Bool = n > 0");
    source.push_str(&" && n > 0".repeat(terms - 1));
    source.push_str(&format!(
        "\ntest \"chains\" {{\n  assert ints() == {terms}\n  assert texts().len() == {terms}\n  assert both(1)\n  assert !both(0)\n}}\n"
    ));
    let dir = out_dir(&format!("{name}-source-{terms}"));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("chains.ignite");
    std::fs::write(&file, source).unwrap();
    file
}

/// The 10,000-term chains lower, build and run on the LLVM target: the one
/// chain test of the gate that builds and runs (slice 11; the 100,000-term
/// builds are the `#[ignore]` tests below, run by hand).
#[test]
fn long_chains_run_on_llvm() {
    assert_passes(&chains(10_000, "chains"), "chains-10000", &["llvm"]);
}

/// Lowering the chains to IR: a `Text` chain appends into one buffer
/// through a constant number of stack slots, and the whole lowering stays
/// under `seconds` of wall time.
fn assert_chains_lower(terms: usize, seconds: f64) {
    let file = chains(terms, "chains-lower");
    let start = std::time::Instant::now();
    let (_, generated) = ignite::emit_llvm(&file, None).unwrap_or_else(|e| panic!("{e}"));
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs_f64() < seconds,
        "lowering {terms} terms took {elapsed:?}"
    );
    let texts = generated
        .program
        .split("define void @\"main.texts\"")
        .nth(1)
        .expect("texts is defined");
    let body = &texts[..texts.find("\n}\n").expect("end of texts")];
    assert!(body.matches("alloca").count() < 10, "{}", &body[..2000]);
}

/// The 10,000-term lowering of the gate.
#[test]
fn long_chains_lower_quickly() {
    assert_chains_lower(10_000, 5.0);
}

/// Lowering 100,000-term chains to IR neither overflows the stack nor
/// takes 20 s of wall time (measured 3.1 s alone, 6.7 s under two
/// parallel gates, 3.5 s in the slice-10 gate); run by hand beside the
/// 100,000-term build below.
#[test]
#[ignore]
fn long_chains_100k_lower_quickly() {
    assert_chains_lower(100_000, 20.0);
}

/// The 100,000-term build and run; prints its time.
#[test]
#[ignore]
fn long_chains_100k_run_on_llvm() {
    let file = chains(100_000, "chains");
    let start = std::time::Instant::now();
    assert_passes(&file, "chains-100000", &["llvm"]);
    eprintln!("100,000-term chains built and ran in {:?}", start.elapsed());
}

/// The newest `libignite_rt-*.a` beside the test executable that has the
/// test runner (as in `ignite_test.rs`).
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
