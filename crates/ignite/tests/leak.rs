//! The census and the soak end to end (`docs/slice.md` 32,
//! `docs/leak-guard.md` 7): the soak tests of `fixtures/leak/seeded.ignite`
//! pass on both targets, and each fails with `IGNITE_DEBUG_LEAK=<kind>`
//! naming the kind that grew (five soaks are known failures on the LLVM
//! target, [`KNOWN_ON_LLVM`]); the G30 soaks of `fixtures/leak/g30.ignite`
//! pass on the Rust target and are red on the LLVM target, naming `handles`
//! and `channels`, until `story:channel-release-on-close` moves them into
//! `std/sync.ignite` green on both.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use crate::common;

const TARGETS: [&str; 2] = ["rust", "llvm"];

/// One `ignite test` per target at a time: the runs of a target share its
/// output and cargo target directories.
static LOCKS: [Mutex<()>; 2] = [Mutex::new(()), Mutex::new(())];

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/leak")
        .join(name)
}

struct Run {
    status: i32,
    output: String,
}

/// `ignite test <file> <filter>` on `target`, with `IGNITE_DEBUG_LEAK` set
/// to `leak` when given.
fn ignite_test(target: &str, file: &str, filter: &str, leak: Option<&str>) -> Run {
    let index = TARGETS.iter().position(|t| *t == target).expect("a target");
    let _guard = LOCKS[index].lock().unwrap_or_else(|e| e.into_inner());
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("leak")
        .join(target);
    let mut command = Command::new(env!("CARGO_BIN_EXE_ignite"));
    command
        .arg("test")
        .arg(fixture(file))
        .arg(filter)
        .args(["--target", target, "--out"])
        .arg(&dir)
        .arg("--runtime")
        .arg(common::runner_archive())
        .env_remove("IGNITE_DEBUG_LEAK");
    if target == "rust" {
        command.arg("--target-dir").arg(dir.join("cargo"));
    }
    if let Some(leak) = leak {
        command.env("IGNITE_DEBUG_LEAK", leak);
    }
    let output = command.output().expect("run ignite test");
    Run {
        status: output.status.code().expect("ignite test exits"),
        output: format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    }
}

/// `f` once per target, each on its own thread.
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
        assert_eq!(failed, 0, "a target failed; its panic is above");
    });
}

/// The seeded leaks: the value of `IGNITE_DEBUG_LEAK`, the soak test that
/// shows it, the census field that must grow, and whether the kind exists
/// on the Rust target.
const SEEDS: [(&str, &str, &str, bool); 12] = [
    ("task", "soak: task", "tasks", true),
    ("channel", "soak: channel", "channels", true),
    ("listener", "soak: listener", "listeners", true),
    ("stream", "soak: stream", "streams", true),
    ("pipe", "soak: pipe", "pipes", true),
    ("child", "soak: child", "children", true),
    ("directory", "soak: directory", "directories", true),
    ("capability", "soak: capability", "capabilities", true),
    ("bytes", "soak: bytes", "heap_bytes", true),
    ("handle", "soak: channel", "handles", false),
    ("heap_copy", "soak: channel", "heap_copies", false),
    ("arena", "soak: arena", "arena_bytes", false),
];

/// The soaks that are red on the LLVM target without a seeded leak, and the
/// kind each names: the LLVM lowering releases only channel ends, pipe ends
/// and streams a block owns (`crates/ignite-llvm/src/own.rs`), so every
/// listener, directory and capability (a launcher, an entropy source) stays
/// in the handle table until the process exits (`docs/leak-guard.md` 1.2,
/// Q8). Known failures: a fix of that class turns these into passes.
const KNOWN_ON_LLVM: [(&str, &str); 5] = [
    ("soak: listener", "listeners"),
    ("soak: stream", "listeners"),
    ("soak: child", "capabilities"),
    ("soak: directory", "directories"),
    ("soak: capability", "capabilities"),
];

fn known_on_llvm(test: &str) -> bool {
    KNOWN_ON_LLVM.iter().any(|(known, _)| *known == test)
}

#[test]
fn every_soak_passes_without_a_seeded_leak() {
    on_both_targets(|target| {
        let run = ignite_test(target, "seeded.ignite", "", None);
        if target == "rust" {
            assert_eq!(run.status, 0, "{target}:\n{}", run.output);
            assert!(
                run.output.contains("11 passed; 0 failed"),
                "{target}:\n{}",
                run.output
            );
            return;
        }
        assert!(
            run.output.contains("6 passed; 5 failed"),
            "{target}: all but the known failures pass:\n{}",
            run.output
        );
        for (test, kind) in KNOWN_ON_LLVM {
            let run = ignite_test(target, "seeded.ignite", test, None);
            assert_eq!(run.status, 1, "{target}, {test}:\n{}", run.output);
            assert!(
                run.output.contains(&format!("kind: \"{kind}\"")),
                "{target}, {test} names no {kind}:\n{}",
                run.output
            );
        }
    });
}

#[test]
fn each_seeded_leak_fails_its_soak_naming_the_kind() {
    on_both_targets(|target| {
        for (leak, test, kind, on_rust) in SEEDS {
            if (target == "rust" && !on_rust) || (target == "llvm" && known_on_llvm(test)) {
                continue;
            }
            let run = ignite_test(target, "seeded.ignite", test, Some(leak));
            assert_eq!(
                run.status, 1,
                "{target}, IGNITE_DEBUG_LEAK={leak}:\n{}",
                run.output
            );
            assert!(
                run.output.contains(&format!("kind: \"{kind}\"")),
                "{target}, IGNITE_DEBUG_LEAK={leak} names no {kind}:\n{}",
                run.output
            );
        }
    });
}

/// G30 (`docs/leak-guard.md` 6.1): green on the Rust target; on the LLVM
/// target a known failure until `story:channel-release-on-close`, which
/// fixes the lowering, turns this case's LLVM half into a pass and moves
/// the two tests into `std/sync.ignite`.
#[test]
fn g30_soaks_pass_on_rust_and_are_known_failures_on_llvm() {
    on_both_targets(|target| {
        let run = ignite_test(target, "g30.ignite", "", None);
        if target == "rust" {
            assert_eq!(run.status, 0, "{target}:\n{}", run.output);
            assert!(
                run.output.contains("2 passed; 0 failed"),
                "{target}:\n{}",
                run.output
            );
            return;
        }
        assert_eq!(run.status, 1, "{target}:\n{}", run.output);
        assert!(
            run.output.contains("0 passed; 2 failed"),
            "{target}:\n{}",
            run.output
        );
        for kind in ["handles", "channels"] {
            assert!(
                run.output.matches(&format!("kind: \"{kind}\"")).count() == 2,
                "{target}: both G30 soaks name {kind}:\n{}",
                run.output
            );
        }
    });
}
