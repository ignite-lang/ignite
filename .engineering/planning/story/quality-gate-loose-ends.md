---
format: aep.planning-md/3
id: story:quality-gate-loose-ends
kind: story
status: draft
title: Loose ends of the quality gate found in wave 11
relations:
- informed_by: story:release-refuses-every-worse-row
revision: 1
---
# Story: loose ends of the quality gate found in wave 11

## Outcome

Eight small gaps in the gate and its counters, each found by an adversary pass of wave 11 and each pre-existing, are closed or stated as a limit in `docs/quality.md`.

## Why

- `crates/ignite-types/Cargo.toml:11` (unit dup, pass 1, finding 3): `[lib] test = false` means the three tests of `builtins.rs` never run; they pass with `--lib`.
- Row 1.3's counter (unit dup, diagnosis): names under `use Builtin::*` and `use PatternKind::*` are invisible to it. The llvm crate names 149 builtin variants as bare words (133 at v0.15.0, a word-match upper bound), so a lower llvm/builtins cell does not show fewer names.
- `tools/ignite-perf/src/bench.rs:184` (unit gate, pass 2, finding 6): a release run with `--bench-passes 5` judges a benchmark row on as few as one sample per side when passes have no clean run.
- `tools/ignite-perf/src/remote.rs:1845` (unit gate, pass 2, finding 7): only the `RUN=v*` label makes a dedicated-host run a release run, and no release step requires a `v*` dedicated-host record before tagging.
- The size rule (unit sizerule, both rounds): a decision from two releases back, replaced in between, reads as new; the lines of `remote::main` and of `quality::record` that print the list of grown cells are held by tests of the text, not of the printing.
- `crates/ignite-io/src/sched.rs:631` (unit unsafe, pass 1, finding 4): the race fix `07c4f9aa` is correct and no test fails without it; a deterministic case needs a hook in `start_worker`.
- `crates/ignite/tests/llvm_build.rs:417` (unit handles, pass 1, finding 5): `release_tests_link_the_native_runtime` fails after `task build` ("runtime archive without the test runner").
- Unit build, pass 2: `PROCESS_ITEMS` in `crates/ignite-rustgen/src/lib.rs:7474` holds `ChildExit`, which the generator never emits, and `Invocation`, which it emits only beside a named `Launcher`. The modelled Rust dev build on one core is 4.3 % above v0.15.0 (24,831.7 M instructions against 23,815.6 M) and below it from two cores up. The driver's `-g0` for third-party C code overrides an `[env] HOST_CFLAGS` of a cargo configuration file.
- Two notes of the wave are kept here by name only and need to be measured again before work starts: the checker's `stores` rule for a user function named `put` on a receiver whose type is not known (remainder of unit counts, pass 1, finding 3), and the alias symbols of the TLS library in a program that opens no TLS session (noted as 33 and 25 for the two targets; the method is not recorded).

## Acceptance

- Each item is either fixed with a test that fails without the fix, or written into `docs/quality.md` or `docs/slice.md` as a stated limit with its reason.
- The tests of `builtins.rs` run in `task gate`.
- A release step of `AGENTS.md` § Releases names the dedicated-host record the tag needs.
- `task gate` green; no row of `docs/quality.md` worse.

## Scope

- `crates/ignite-types/Cargo.toml`, `crates/ignite/tests/quality.rs`, `tools/ignite-perf/src/bench.rs`, `tools/ignite-perf/src/remote.rs`, `crates/ignite-io/src/sched.rs`, `crates/ignite/tests/llvm_build.rs`, `crates/ignite-rustgen/src/lib.rs`, `docs/quality.md`, `AGENTS.md`. Inferred.
