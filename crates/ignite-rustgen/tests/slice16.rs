//! Slice 16 on the Rust target (`docs/slice.md` 20.3.4, 20.0 rows 9 and
//! 11): the fixtures of `crates/ignite/tests/fixtures/slice16/` are lowered
//! as test builds against the repository's `std/`, built with cargo and
//! run. Row 9: R1's state threaded through a call is linear once the state
//! moves. Row 11: the moves and the calls of fn-typed expressions build
//! with rustc (no E0382, E0505 or E0507) and keep value semantics.

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
    common::workspace()
        .join("crates/ignite/tests/fixtures/slice16")
        .join(name)
}

/// Lower `file` as a test build (dev profile), build it with cargo and
/// return its `ignite-test` executable.
fn build_tests(file: &Path, name: &str) -> PathBuf {
    let substrate = common::workspace().join("crates/ignite-io");
    let substrate = substrate.canonicalize().unwrap_or(substrate);
    let (project, std) = common::lower_tests(file, &common::repository_std(), &substrate);
    let target = target_dir();
    let out = target.join("ignite-rustgen-slice16").join(name);
    write_project(&project, &out).unwrap();
    write_project(&std, &out.join("std")).unwrap();
    let builds = target
        .join("ignite-rustgen-slice16")
        .join(format!("{name}-target"));
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .arg("build")
        .arg("--manifest-path")
        .arg(out.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(&builds)
        .env("CARGO_INCREMENTAL", "0")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "cargo build of {} failed:\n{}",
        out.display(),
        text(&output)
    );
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

/// The full name of the one test whose name ends in `/<title>`, from the
/// runner's `--list` (one `<name>: test` line per test).
fn full_name(executable: &Path, title: &str) -> String {
    let output = run(executable, &["--list"]);
    assert!(output.status.success(), "{}", text(&output));
    let listed = String::from_utf8_lossy(&output.stdout).into_owned();
    let suffix = format!("/{title}");
    let names: Vec<&str> = listed
        .lines()
        .filter_map(|line| line.strip_suffix(": test"))
        .filter(|name| name.ends_with(&suffix))
        .collect();
    assert_eq!(names.len(), 1, "one `{title}` in\n{listed}");
    names[0].to_string()
}

/// The `finished in` seconds of the runner's `test result:` line, with the
/// line checked for one passed test and none failed.
fn finished_in(output: &Output) -> f64 {
    let shown = text(output);
    assert!(output.status.success(), "{shown}");
    let line = shown
        .lines()
        .find(|line| line.starts_with("test result:"))
        .unwrap_or_else(|| panic!("no `test result:` line in\n{shown}"));
    assert!(
        line.contains(" 1 passed; 0 failed;"),
        "not exactly one test passed: {line}"
    );
    let seconds = line
        .rsplit("finished in ")
        .next()
        .and_then(|rest| rest.strip_suffix('s'))
        .and_then(|number| number.trim().parse::<f64>().ok())
        .unwrap_or_else(|| panic!("no `finished in` figure in: {line}"));
    eprintln!("{line}");
    seconds
}

/// Row 9: `s = step(s, i)` moves `s`, so 200,000 steps take under eight
/// times what 50,000 take (linear is about 4; a copy per step, p07's
/// shape, about 20 per factor 4).
#[test]
fn state_threading_is_linear() {
    let executable = build_tests(&fixture("moves_scale.ignite"), "moves_scale");
    let small = full_name(&executable, "50000 steps through a call");
    let large = full_name(&executable, "200000 steps through a call");
    let small = finished_in(&run(&executable, &[&small, "--exact"]));
    let large = finished_in(&run(&executable, &[&large, "--exact"]));
    assert!(
        large < 8.0 * small.max(0.01),
        "200000 steps took {large:.2}s, 50000 took {small:.2}s: not linear"
    );
}

/// Rows 11 and 18: `moves.ignite`, `calls.ignite`,
/// `moves_tails_untracked.ignite`, `moves_tails_owned.ignite` and `rest.ignite` build with rustc and every test of each passes. The passed count is each
/// fixture's number of tests, so a fixture that runs none fails.
#[test]
fn fixtures_pass() {
    for (name, tests) in [
        ("moves", 13),
        ("calls", 6),
        ("moves_tails_untracked", 2),
        ("moves_tails_owned", 8),
        ("rest", 7),
    ] {
        let executable = build_tests(&fixture(&format!("{name}.ignite")), name);
        let output = run(&executable, &[]);
        let shown = text(&output);
        assert!(output.status.success(), "{name}: {shown}");
        assert!(
            shown.contains(&format!("test result: ok. {tests} passed; 0 failed;")),
            "{name}: {shown}"
        );
    }
}

/// Row 11 (adversary, U3): a consumed parameter and a local that are the
/// tail of a branch or an arm whose value is used, and are read afterwards,
/// build with rustc (no E0382) and every test passes.
#[test]
fn branch_tails_build_and_pass() {
    for name in ["moves_tails", "moves_tails_local"] {
        let executable = build_tests(&fixture(&format!("{name}.ignite")), name);
        let output = run(&executable, &[]);
        let shown = text(&output);
        assert!(output.status.success(), "{name}: {shown}");
        assert!(shown.contains(" 3 passed; 0 failed;"), "{name}: {shown}");
    }
}
