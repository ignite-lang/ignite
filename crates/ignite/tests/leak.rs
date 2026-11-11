//! The census and the soak end to end (`docs/slice.md` 33,
//! `docs/leak-guard.md` 7): the soak tests of `fixtures/leak/seeded.ignite`
//! pass on both targets, and each fails with `IGNITE_DEBUG_LEAK=<kind>`
//! naming the kind that grew. The G30 soaks are inline tests of
//! `std/sync.ignite`; the case study's reproducer of G30,
//! `fixtures/leak/repro.ignite`, stays flat in every mode on both targets.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
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

/// A soak program outside the differential runner's corpus
/// (`tests/soak/`): its tests read `Runtime.census()`, which the evaluator
/// has no model of by design, so in `fixtures/` each would be a unit of
/// `eval_cannot_run` (metric 1.6, `docs/quality.md`).
fn soak_program(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/soak")
        .join(name)
}

struct Run {
    status: i32,
    output: String,
}

/// `ignite test <file> <filter>` on `target` for a fixture of
/// `fixtures/leak/`, with `IGNITE_DEBUG_LEAK` set to `leak` when given.
fn ignite_test(target: &str, file: &str, filter: &str, leak: Option<&str>) -> Run {
    ignite_test_at(target, &fixture(file), filter, leak)
}

/// `ignite test <path> <filter>` on `target` (see [`ignite_test`]).
fn ignite_test_at(target: &str, path: &Path, filter: &str, leak: Option<&str>) -> Run {
    let index = TARGETS.iter().position(|t| *t == target).expect("a target");
    let _guard = LOCKS[index].lock().unwrap_or_else(|e| e.into_inner());
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("leak")
        .join(target);
    let mut command = Command::new(env!("CARGO_BIN_EXE_ignite"));
    command
        .arg("test")
        .arg(path)
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

/// Every soak passes on both targets: since `story:llvm-release-all-handles`
/// the LLVM lowering releases the listeners, directories and capabilities a
/// block owns too (`crates/ignite-llvm/src/own.rs`).
#[test]
fn every_soak_passes_without_a_seeded_leak() {
    on_both_targets(|target| {
        let run = ignite_test(target, "seeded.ignite", "", None);
        assert_eq!(run.status, 0, "{target}:\n{}", run.output);
        assert!(
            run.output.contains("11 passed; 0 failed"),
            "{target}:\n{}",
            run.output
        );
    });
}

#[test]
fn each_seeded_leak_fails_its_soak_naming_the_kind() {
    on_both_targets(|target| {
        for (leak, test, kind, on_rust) in SEEDS {
            if target == "rust" && !on_rust {
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

/// The reproducer's modes, `none` (no channel) first.
const MODES: [&str; 6] = ["none", "local", "alias", "typed", "field", "server"];

/// Tasks per run of the reproducer: on v0.15.0 the leaking modes grew by
/// about 10 MB at this count on the LLVM target.
const ITERATIONS: &str = "20000";

/// How far a mode's peak resident size may lie above `none`'s.
const SLACK_KB: i64 = 2 * 1024;

/// Runs of a mode at most; see [`g30_reproducer_stays_flat_in_every_mode`].
const RUNS: usize = 3;

/// `VmHWM` of process `pid` in KiB: its peak resident size so far; `None`
/// once it has exited.
fn high_water_kb(pid: u32) -> Option<i64> {
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    status
        .lines()
        .find_map(|line| line.strip_prefix("VmHWM:"))
        .and_then(|rest| rest.trim().trim_end_matches("kB").trim().parse().ok())
}

/// Run `exe` with `MODE=mode` and `N=ITERATIONS`; its exit code and its
/// peak resident size in KiB. The peak is `VmHWM`, which only grows: read
/// when the program logs that it is done, after every task ran, and every
/// 2 ms before that. `wait4`'s `ru_maxrss` cannot be used: it counts the
/// test process's own size, which the child had before its `exec`.
fn peak_kb(exe: &Path, mode: &str) -> (i32, i64) {
    use std::io::{BufRead, BufReader};
    use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
    let mut child = Command::new(exe)
        .env("MODE", mode)
        .env("N", ITERATIONS)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("start {}: {e}", exe.display()));
    let pid = child.id();
    let peak = AtomicI64::new(0);
    let ended = AtomicBool::new(false);
    let note = |kb: Option<i64>| {
        if let Some(kb) = kb {
            peak.fetch_max(kb, Ordering::Relaxed);
        }
    };
    let mut lines = Vec::new();
    std::thread::scope(|scope| {
        scope.spawn(|| {
            while !ended.load(Ordering::Relaxed) {
                note(high_water_kb(pid));
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        });
        let streams = [
            Box::new(child.stdout.take().expect("piped stdout")) as Box<dyn std::io::Read + Send>,
            Box::new(child.stderr.take().expect("piped stderr")),
        ];
        let readers: Vec<_> = streams
            .into_iter()
            .map(|stream| {
                scope.spawn(|| {
                    let mut seen = Vec::new();
                    for line in BufReader::new(stream).lines().map_while(Result::ok) {
                        if line.ends_with(" done") {
                            note(high_water_kb(pid));
                        }
                        seen.push(line);
                    }
                    seen
                })
            })
            .collect();
        for reader in readers {
            lines.extend(reader.join().expect("a reader"));
        }
        ended.store(true, Ordering::Relaxed);
    });
    let status = child.wait().expect("wait for the reproducer");
    assert!(
        lines
            .iter()
            .any(|line| line.ends_with(&format!("{mode}: {ITERATIONS} done"))),
        "{mode} did not finish: {lines:?}"
    );
    (status.code().unwrap_or(-1), peak.load(Ordering::Relaxed))
}

/// Story `channel-release-on-close`: the case study's reproducer builds
/// on each target and every mode at `N=20000` peaks within 2 MB of
/// `none`, the mode that opens no channel.
#[test]
fn g30_reproducer_stays_flat_in_every_mode() {
    let source = fixture("repro.ignite");
    let targets = [common::Target::Rust, common::Target::Llvm];
    std::thread::scope(|scope| {
        let handles: Vec<_> = targets
            .iter()
            .map(|target| {
                let source = &source;
                scope.spawn(move || {
                    let options =
                        target.options(&Path::new(env!("CARGO_TARGET_TMPDIR")).join("leak-repro"));
                    let output = ignite::build(source, &options).unwrap_or_else(|e| panic!("{e}"));
                    let exe = output
                        .executables
                        .iter()
                        .find(|p| p.file_stem().and_then(|s| s.to_str()) == Some("repro"))
                        .unwrap_or_else(|| panic!("no repro in {:?}", output.executables))
                        .clone();
                    // A run whose tasks happen to land on a second pooled
                    // worker peaks about 2 MB higher (its stack and arena
                    // chunk), in any mode `none` included; a leak shows in
                    // every run. So each mode's peak is the lowest of up
                    // to `RUNS` runs, `none`'s of all of them.
                    let mut peaks: Vec<(&str, Vec<i64>)> = Vec::new();
                    let mut base = i64::MAX;
                    for mode in MODES {
                        let mut runs = Vec::new();
                        while runs.len() < RUNS {
                            let (code, kb) = peak_kb(&exe, mode);
                            assert_eq!(code, 0, "{target:?}: {mode} exits 0");
                            runs.push(kb);
                            if mode == "none" {
                                base = base.min(kb);
                            } else if kb - base <= SLACK_KB {
                                break;
                            }
                        }
                        peaks.push((mode, runs));
                    }
                    let report = format!("{target:?}, peak KiB per mode and run: {peaks:?}");
                    for (mode, runs) in &peaks {
                        let low = runs.iter().copied().min().unwrap_or(i64::MAX);
                        assert!(
                            low - base <= SLACK_KB,
                            "{mode} peaks {} KiB above none; {report}",
                            low - base
                        );
                    }
                })
            })
            .collect();
        let failed = handles
            .into_iter()
            .map(|handle| handle.join())
            .filter(Result::is_err)
            .count();
        assert_eq!(failed, 0, "a target failed; its panic is above");
    });
}

/// Adversary cases (`fixtures/leak/attack.ignite`): a handle the LLVM
/// lowering releases is still usable wherever it is reachable (an alias or
/// a record field of an end handed to a task, a stored end read back, an
/// end sent through a channel), and a server's broadcast over stored
/// senders and a look at one through a temporary keep the census flat.
#[test]
fn reachable_handles_stay_usable_and_stored_reads_are_released() {
    on_both_targets(|target| {
        let run = ignite_test(target, "attack.ignite", "", None);
        assert_eq!(run.status, 0, "{target}:\n{}", run.output);
        assert!(
            run.output.contains("6 passed; 0 failed"),
            "{target}:\n{}",
            run.output
        );
    });
}

/// Adversary cases, pass 2 (`fixtures/leak/attack2.ignite`): an aliased end
/// handed to a task stays usable through nested records and branches, an end
/// a broadcast hands to a task stays the task's, and the stored ends a server
/// reads through `for` (left by `break`, `continue`, `return`), nested
/// `match`es, `is_some`, a bare `remove` or a record literal `put` keep the
/// census flat.
#[test]
fn stored_ends_read_in_every_form_are_released() {
    on_both_targets(|target| {
        let run = ignite_test(target, "attack2.ignite", "", None);
        assert_eq!(run.status, 0, "{target}:\n{}", run.output);
        assert!(
            run.output.contains("16 passed; 0 failed"),
            "{target}:\n{}",
            run.output
        );
    });
}

/// A stored end read into a temporary no binding holds
/// (`tests/soak/temporaries.ignite`): consumed by a fn's result or
/// returned by it, passed to `spawn`, read in a loop condition, taken by a
/// call under `?` or beside a `?` that leaves, and pending in a task that
/// fails. Each case fails on a copy never released (the soak's census) and
/// on a copy released twice (the stored end stops delivering).
#[test]
fn stored_ends_read_into_temporaries_are_released_once() {
    on_both_targets(|target| {
        let run = ignite_test_at(target, &soak_program("temporaries.ignite"), "", None);
        assert_eq!(run.status, 0, "{target}:\n{}", run.output);
        assert!(
            run.output.contains("5 passed; 0 failed"),
            "{target}:\n{}",
            run.output
        );
    });
}

/// Adversary cases (`tests/soak/temporaries_attack.ignite`): the store lets
/// its own owner go before the temporary is used, so a copy released before
/// its use or twice closes the end the temporary names; a temporary sent,
/// put, passed to `spawn` directly or through a callee, aliased by a fn's
/// result, on every path of `if`, `break`, `continue` and a short circuit,
/// and a stream passed to a role call. A fresh end a callee moves to a task
/// stays open for the task whether the callee rebinds it through a fn, a
/// method, a role, a generic fn, a fn value, a field, an `if` or a `match`
/// of calls, and whether the caller passes it as a temporary or a binding
/// (`own::moves_fresh`); a call in a loop that passes the same end to a
/// task on every round gives each task an owner of its own
/// (`own::sole_mentions`).
#[test]
fn temporaries_used_after_the_store_let_go_are_released_once() {
    on_both_targets(|target| {
        let run = ignite_test_at(target, &soak_program("temporaries_attack.ignite"), "", None);
        assert_eq!(run.status, 0, "{target}:\n{}", run.output);
        assert!(
            run.output.contains("19 passed; 0 failed"),
            "{target}:\n{}",
            run.output
        );
    });
}

/// Adversary cases (`tests/soak/temporaries_unreleased.ignite`): store
/// reads no owner rule covers on LLVM (the receiver of a builtin method,
/// the rest of a fresh value read for one field, a thrown-away match arm
/// value, a read inside a record argument, a match binding put in a store
/// inside `Some`). The Rust target drops them at the end of the statement.
///
/// Known gap, pre-existing (red at `f58a7733` too), filed as
/// `story:llvm-temporaries-without-owner-rule`: on LLVM the five still keep
/// the owner `copy_from_heap` added. This test holds today's state by name,
/// each failing by growth of the handle census, so that a fix of any form,
/// or a form that turns from a leak into an early or double release, turns
/// it red and is taken into the expectation.
#[test]
fn five_temporary_forms_are_not_released_on_llvm_yet() {
    const KNOWN_ON_LLVM: [&str; 5] = [
        "a store read that is the receiver of a builtin method is released",
        "the rest of a fresh value read for one field is released",
        "a store read that is the thrown-away value of a match arm is released",
        "a store read inside a record argument of a call is released",
        "a match scrutinee whose arm puts the binding in a store inside Some is released",
    ];
    on_both_targets(|target| {
        let run = ignite_test_at(
            target,
            &soak_program("temporaries_unreleased.ignite"),
            "",
            None,
        );
        if target == "llvm" {
            assert_ne!(run.status, 0, "{target}:\n{}", run.output);
            assert!(
                run.output.contains("0 passed; 5 failed"),
                "known gap (story:llvm-temporaries-without-owner-rule): {target} changed:\n{}",
                run.output
            );
            for name in KNOWN_ON_LLVM {
                assert!(
                    run.output
                        .contains(&format!("test temporaries_unreleased/{name} ... FAILED")),
                    "known gap (story:llvm-temporaries-without-owner-rule): {name} no longer \
                     fails on {target}:\n{}",
                    run.output
                );
                let failure = failure_of(&run.output, &format!("temporaries_unreleased/{name}"));
                assert!(
                    failure.contains("soak(200,")
                        && failure.contains("Growth { kind: \"handles\", before: "),
                    "known gap (story:llvm-temporaries-without-owner-rule): {name} no longer \
                     fails by growth on {target}:\n{failure}"
                );
            }
        } else {
            assert_eq!(run.status, 0, "{target}:\n{}", run.output);
            assert!(
                run.output.contains("5 passed; 0 failed"),
                "{target}:\n{}",
                run.output
            );
        }
    });
}

/// Adversary cases, pass 2 (`soak/moves_attack.ignite`): an end a `spawn`
/// moves to its task (`own::moves_fresh`, `own::sole_mentions`,
/// `region::moved_to_task`) stays open for every other holder the caller
/// kept: an alias, a record or a closure made before the call, an owned
/// binding with an alias, a parameter bound by an arm's own `let`, a later
/// mention, a match binding and a field of a parameter; and the owners a
/// task gets are released once per round (the `over_pipe` shape of the
/// golden `bounds.program.ll`, an end of an outer block, a stream a role
/// call returns from an accepted temporary). The case of a list the callee
/// reads with `at` is a known gap
/// ([`an_element_read_from_a_list_and_moved_to_a_task_is_closed_early_on_llvm_yet`]).
#[test]
fn ends_moved_to_a_task_stay_open_for_every_other_holder() {
    on_both_targets(|target| {
        let run = ignite_test_at(target, &soak_program("moves_attack.ignite"), "", None);
        assert_eq!(run.status, 0, "{target}:\n{}", run.output);
        assert!(
            run.output.contains("11 passed; 0 failed"),
            "{target}:\n{}",
            run.output
        );
    });
}

/// Adversary case, pass 2, moved unchanged to `soak/moves_aliases.ignite`:
/// a callee reads an end from a list with `xs.at(0)` and spawns it.
///
/// Known gap, pre-existing (red at `f58a7733` and at v0.15.0 too), filed as
/// `story:llvm-task-move-aliases`: `own::moves_fresh` calls the result of
/// every builtin other than `copy()` fresh, so the task takes the list's
/// owner of the end without one of its own and its release closes the end
/// the caller still sends on. This test holds the LLVM failure, so a fix
/// turns it red and the case goes back to the adversary's test.
#[test]
fn an_element_read_from_a_list_and_moved_to_a_task_is_closed_early_on_llvm_yet() {
    const NAME: &str = "moves_aliases/an element a callee reads from a list and moves to a task \
                        stays open for the list";
    on_both_targets(|target| {
        let run = ignite_test_at(target, &soak_program("moves_aliases.ignite"), "", None);
        if target == "llvm" {
            assert_ne!(run.status, 0, "{target}:\n{}", run.output);
            assert!(
                run.output.contains("0 passed; 1 failed")
                    && run.output.contains(&format!("test {NAME} ... FAILED")),
                "known gap (story:llvm-task-move-aliases): {target} changed:\n{}",
                run.output
            );
            let failure = failure_of(&run.output, NAME);
            assert!(
                failure.contains("is not a channel sender"),
                "known gap (story:llvm-task-move-aliases): {target} no longer fails by an early \
                 release:\n{failure}"
            );
        } else {
            assert_eq!(run.status, 0, "{target}:\n{}", run.output);
            assert!(
                run.output.contains("1 passed; 0 failed"),
                "{target}:\n{}",
                run.output
            );
        }
    });
}

/// The failure report of the test `name` in the output of `ignite test`:
/// from its `---- <name> stderr ----` line to the next report or the
/// failure list; empty when there is none.
fn failure_of<'o>(output: &'o str, name: &str) -> &'o str {
    let head = format!("---- {name} stderr ----");
    let Some(start) = output.find(&head) else {
        return "";
    };
    let body = &output[start + head.len()..];
    let end = ["\n---- ", "\nfailures:"]
        .iter()
        .filter_map(|marker| body.find(marker))
        .min()
        .unwrap_or(body.len());
    &body[..end]
}
