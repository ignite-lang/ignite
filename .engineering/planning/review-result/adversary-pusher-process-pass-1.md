---
format: aep.planning-md/3
id: review-result:adversary-pusher-process-pass-1
kind: review-result
status: active
title: 'Adversary pass 1: case-study-process-builtins'
relations:
- reviews: story:pusher-process-builtins
revision: 2
---
unit: story:case-study-process-builtins, worktree case-study-process, commit 0b990e67 plus one uncommitted test file
verdict: CONFIRMED (warning: a mutant the suite missed; no red case)
cases: executed 7→9 (`process_builtins::`), red 0
origin: introduced 2 / pre-existing 1 / undecided 0
wrote-outside-worktree: scratch unit-case-study-process/adv1 (104 K)
needs-coordinator: none

## Case

- `process_builtins::{rust,llvm}::g2_launch_skips_a_variable_that_is_not_utf8`: green; kills the mutant `Environment::launch` on `std::env::vars()` (panic, exit 101), which the existing suite let through. Committed by the coordinator as `8a5d6cfc`.

## Findings

- F1 (warning) `crates/ignite-io/src/environment.rs:39`: the launch's skip of non-UTF-8 variables, promised in `docs/slice.md` 21.20, had no end-to-end test.
- F2 (note, pre-existing, infeasible) `crates/ignite-io/src/net.rs:212`: `Clock.sleep` and `Process.flush` send buffered writes with a blocking `write_all`; a stalled peer blocks the sleep. Matches section 7's wait points; 21.20 does not say so.
- F3 (note) `crates/ignite/tests/fixtures/process_builtins/case-study_gaps.ignite:53`, `crates/ignite-rt/src/process.rs:712`: two flush tests assert nothing; the behaviour is covered by the `received_batches` assertions in `std/process.ignite`.

## Not broken

E016/E017/E042/E051/E116/E010 effect and placement rules; Rust and LLVM agree on odd names and values, 200 KB values, environments inside records, stores, maps and lists, `Clock.now` against `std.time`, sleeps of 0 and negative durations, flush twice and with nothing pending; `plan`, `explain`, `emit --typed-json`, `run --clock manual` and `replay`.

## Coordinator disposition

F1 closed by `8a5d6cfc`. F2 and F3 are notes, no change in this story. No second adversary pass: no red case and no code change.

```json
[
  {"id": "F1", "severity": "warning", "title": "Launch skip of variables that are not UTF-8 had no end-to-end test (a mutant the suite misses)", "evidence": "crates/ignite-io/src/environment.rs:39; mutant std::env::vars() makes g2 panic, exit 101", "test": "process_builtins::{rust,llvm}::g2_launch_skips_a_variable_that_is_not_utf8", "pre_existing": false},
  {"id": "F2", "severity": "note", "title": "Clock.sleep and Process.flush block indefinitely on a stalled TCP peer", "evidence": "crates/ignite-io/src/net.rs:212 blocking write_all, no send timeout", "test": null, "pre_existing": true},
  {"id": "F3", "severity": "note", "title": "Flush tests with no assertion", "evidence": "case-study_gaps.ignite:53, ignite-rt process.rs:712", "test": null, "pre_existing": false}
]
```
