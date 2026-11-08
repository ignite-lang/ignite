---
format: aep.planning-md/3
id: story:quality-ratchet
kind: story
status: implemented
title: Quality metrics with a ratchet; one result across Rust, LLVM and the evaluator
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: Taskfile.yml
- confidence: inferred
  path: crates/ignite/tests/differential.rs
- confidence: inferred
  path: crates/ignite/tests/main.rs
- confidence: inferred
  path: crates/ignite/tests/quality.rs
- confidence: inferred
  path: crates/ignite/tests/site.rs
- confidence: inferred
  path: docs/quality.md
- confidence: inferred
  path: quality/baseline.json
- confidence: inferred
  path: quality/history.jsonl
- confidence: inferred
  path: quality/trend.md
- confidence: inferred
  path: site/book/src/SUMMARY.md
- confidence: inferred
  path: site/book/src/quality.md
- confidence: inferred
  path: tools/ignite-perf/src
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-07T19:49:39Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-11-07T19:49:39Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-11-08T03:39:36Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"test_result":1}}}
---
# Story: quality-ratchet

## Outcome

Ignite has a written set of quality metrics with today's values as the bar, and `task gate` fails when a gated metric gets worse: first of all when Rust, LLVM and the evaluator give a program different results. `story:mid-level-ir` lands behind this gate.

## Context

- Timo, 2026-11-07: "before we do such a change, we need to define some quality gates ... set a bar, and the goal would be to never go below the status quo. We can also use this for hill climbing." Option A: this story takes wave 5's slot of `mid-level-ir`, which moves to wave 6.
- The metric list, 37 rows in 8 areas with kind (gate, ratchet, hill-climb), instrument and status quo on `ab1e597a`: `.engineering/waves/quality-metrics-2026-11-07.md`.
- Pattern to follow: `ux/baseline.json` and `site::ux::ratchet` (lower- or higher-is-better values with a tolerance; an improvement is written into the baseline in the same merge; a worse baseline needs a decision id).
- Today: 71 `on_both_targets` call sites compare Rust and LLVM; the evaluator is compared only in `crates/ignite/tests/fixtures/eval_equivalence.ignite` and `control_trace`.

## Acceptance

- `docs/quality.md` holds the metric list (from the file above), each row with its kind, instrument and bar; `AGENTS.md` names it.
- The website documents them (Timo, 2026-11-07): a book page under `site/book/src/` (linked from `SUMMARY.md`) lists every metric per area and target with its bar, its current value and its trend, generated or checked from `quality/` so it cannot drift (a `site::` test), and passes `site::` and `task ux`.
- A differential runner runs a corpus of programs (at least every `crates/ignite/tests/fixtures/**/*.ignite` with a `main` or tests the evaluator can run) on Rust, LLVM and the evaluator and compares exit, stdout and failure text; it reports per program and fails on a difference. Programs the evaluator cannot run are listed with the reason, and their count is a ratchet.
- `quality/baseline.json` holds every ratchet row that has an instrument after this story (at least: differential corpus size and differences, diagnostic codes without a fixture, `#[ignore]` count, inline tests per target, conformance claims and gaps, rule implementation count for pattern matching and builtins, `unsafe` sites in `ignite-rt` and `ignite-io`), and a test in `crates/ignite` fails on a worse value, an untaken improvement or a missing value, as the UI ratchet does.
- Every metric that differs by target is recorded per target: `rust`, `llvm` and, where it runs, `eval` (Timo, 2026-11-07: "per kind, the Rust variant and the pure LLVM variant").
- History: one command (`task quality`) measures every instrumented metric once, including the timing rows of `tools/ignite-perf` with their load, and appends one record per run to `quality/history.jsonl` (commit, date, target, metric, value); `quality/trend.md` shows one row per release and the change against the last.
- The first assessment is run and committed by this story: today's values are the bar.
- Release gate: the release steps in `AGENTS.md` § Releases run `task quality` and refuse the release when a gated metric is worse than at the last release beyond its tolerance; the release notes carry the trend row.
- Hill-climb: `docs/quality.md` names a target per metric where one is set; a wave that moves a metric records it in its close.
- `task gate` runs the differential runner and the ratchet; timing rows are measured by `task quality`, not gated per merge.
- `task gate` green.

## Out of Scope

- Fuzzing and a program generator (metrics 1.5, 2.4), mutation testing (7.5), load baselines (3.3): rows in `docs/quality.md` marked unmeasured.
- Fixing any metric: the bar is today's value.
