---
format: aep.planning-md/3
id: story:eval-substrate-model
kind: story
status: implemented
title: The evaluator runs tests over in-memory stores, pipes, tasks and channels
scope:
- confidence: cited
  path: crates/ignite-eval/src
- confidence: cited
  path: crates/ignite/tests/differential.rs
- confidence: cited
  path: docs/slice.md
- confidence: inferred
  path: quality/baseline.json
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-11T00:24:54Z", actor: "human:timo", revision: 4, executor: "agent:claude-coordinator", correlation: "wave-11"}
- {from: "proposed", to: "active", at: "2026-11-11T00:24:54Z", actor: "human:timo", revision: 5, executor: "agent:claude-coordinator", correlation: "wave-11"}
- {from: "active", to: "implemented", at: "2026-11-11T23:22:41Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1,"review_outcome":2}}}
---
# Story: the evaluator runs tests over in-memory stores, pipes, tasks and channels

## Outcome

Row 1.6 (corpus units the evaluator cannot run) is at or below 236, its v0.15.0 value (now 362),
because the evaluator runs those tests, with a deterministic in-memory model of the substrate
that is active only when a test runs.

## Why

Timo, 2026-11-10, asked whether 1.6 blocks v0.16.0 until it is back at 236: "A" (it blocks). 126
units were added since v0.15.0 and none left; the recorded reasons are `PipePair` (173 of 362),
`StoreNew` (50), `TcpListenerBind` (31), `RuntimeCensus` (29), `ChannelBounded` (15).

Diagnosis (`.engineering/waves/2026-11-11-wave-11-diagnosis/eval.md`), a static scan of what every
listed unit reaches, calibrated by a prototype that ran 29 of 29 predicted units:

- No set of models without task spawn gets below 251.
- Cumulative, in build order: module field 361; store 337; capabilities 333; pipe 307; tasks and
  protocol role calls 240; channel 173; monotonic clock and sleep 157.
- Out by design: the census, entropy, temporary directories, TCP and TLS, child processes,
  stdio, `Process.exit`, executables whose `main` does IO.

## Acceptance

- `eval_cannot_run` holds 236 units or fewer, bar 1.6 `eval` is blessed to the measured value and
  its decision withdrawn; no unit leaves the corpus.
- Every unit that newly runs agrees with the Rust target in the differential runner: row 1.1
  `eval` does not rise above its bar. A unit the model cannot run faithfully is refused with
  `evaluation failed: ...` and stays in the list; it is not made to pass.
- The model is active only under `run_test`: `ignite check` and `ignite plan` still report "has
  no value at compile time" for a property or a system that reaches one of these builtins; a
  test pins this.
- Tasks run on a cooperative run-to-block scheduler in spawn order, the same on every run; a
  deadlock of the model is reported as `evaluation failed`, a spinning task ends on fuel; unit
  tests pin the order, the deadlock report and that a failing task ends only itself.
- Time is virtual: it advances only when every task is blocked, to the earliest deadline. No
  wall-clock read enters the evaluator (row 1.7).
- Pipes deliver at the flush points the targets have (`crates/ignite-io/src/pipe.rs`), so
  `received_batches` agrees; handles are dropped at their last use (`TypedProgram::moves`).
- Metric 1.3 does not rise; the evaluator's node count includes what the model's heap holds.
- `docs/slice.md` 33 states what has a value under a test and what still has none.

## Scope

- `crates/ignite-eval/src/eval.rs`, `value.rs`, `model.rs`, `explain.rs`, a new `substrate.rs`,
  their tests. Cited.
- `crates/ignite/tests/differential.rs` (per-unit limits only if a unit needs one). Cited.
- `docs/slice.md` 33. Cited.
- `quality/baseline.json` (`eval_cannot_run`, bar 1.6): blessed by the unit's run, merged by the
  coordinator.
