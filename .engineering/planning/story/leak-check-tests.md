---
format: aep.planning-md/3
id: story:leak-check-tests
kind: story
status: active
title: ignite test fails a test that leaves a runtime object behind
summary: Default-on per-test leak check on object counts, opt-out with Runtime.leaves_running
relations:
- decomposes: epic:leak-guard
- depends_on: story:leak-census
- depends_on: story:runtime-metrics
- depends_on: story:channel-release-on-close
scope:
- confidence: inferred
  path: crates/ignite-check/src/lib.rs
- confidence: inferred
  path: crates/ignite-io/src/census.rs
- confidence: inferred
  path: crates/ignite-io/src/test.rs
- confidence: inferred
  path: crates/ignite-llvm/src/lower.rs
- confidence: inferred
  path: crates/ignite-rt/src/test.rs
- confidence: inferred
  path: crates/ignite-rustgen/src/lib.rs
- confidence: inferred
  path: crates/ignite-types/src/builtins.rs
- confidence: inferred
  path: crates/ignite-types/src/lib.rs
- confidence: inferred
  path: crates/ignite/src/lib.rs
- confidence: inferred
  path: crates/ignite/tests/ignite_test.rs
- confidence: inferred
  path: docs/slice.md
- confidence: inferred
  path: quality/baseline.json
- confidence: inferred
  path: std/runtime.ignite
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T15:04:31Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-11-08T21:40:02Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"review_outcome":3}}}
---
# Story: leak-check-tests

## Outcome

Every test under `ignite test` is a leak test without code: after its body returns and its tasks end, every compared kind of the census is back at its count before the body, on both targets, or the test fails as `FAILED (leaked N objects)` with a table of the kinds that grew. A test that leaves a server running on purpose says so with `Runtime.leaves_running(reason)`.

## Context

- Design: `docs/leak-guard.md` sections 5.3 (the check in `run_one`), 6.2 and 6.3 (target form and output), Q1 (on by default), Q2 (exit code 102), Q3 (settle bound 2 s), Q8.
- Builds on `story:leak-census`'s counters. Object counts only: bytes are compared only in a soak, so caches and pools give no false positive.
- The runner is `ignite-io`'s for both targets (`crates/ignite-io/src/test.rs`, `docs/slice.md` 12.7).

## Acceptance

- Before the check, the plain test of `crates/ignite/tests/fixtures/leak/` (from `story:leak-census`) passes under `IGNITE_DEBUG_LEAK=channel`; with the check it fails on both targets as `FAILED (leaked 1 object)` with the `channels` row in its failure block, and without the variable it passes.
- Opt-outs: the same leaking test under `IGNITE_DEBUG_LEAK=channel` passes when its body calls `Runtime.leaves_running("…")` and fails without the call; it passes when the run is given `--no-leak-check` and fails without the flag. `Runtime.leaves_running` outside a test is E051, as `Pipe.pair()` (a checker fixture).
- The sweep: `task test-ignite` runs every test of `std/` and `examples/` under the check on both targets, including those `story:runtime-metrics` added; each test that does not pass it is fixed or opts out with a reason; the count of opt-outs is a count row in `quality/baseline.json` (lower is better). A leak found in the sweep that needs a runtime or compiler fix is filed as its own story.
- `docs/slice.md` 12.7 states the check, the verdict, exit code 102 and the flag; `task gate` green.

## Scope

- `crates/ignite-io/src/test.rs`, `crates/ignite-io/src/census.rs`, `crates/ignite-rt/src/test.rs`: the check, the verdict and `--no-leak-check`. Inferred.
- `crates/ignite-check/src/lib.rs`, `crates/ignite-types/src/{lib,builtins}.rs`, `crates/ignite-rustgen/src/lib.rs`, `crates/ignite-llvm/src/lower.rs`: the test-only builtin `Runtime.leaves_running`; `std/runtime.ignite` documents it. Inferred.
- `crates/ignite/src/lib.rs` (the `ignite test` flag), `quality/baseline.json`, `crates/ignite/tests/ignite_test.rs`, a checker fixture for E051, `docs/slice.md`. Inferred.
- The `std/` and `examples/` files that get an opt-out or a fix are found by the sweep and not known now; ordered after `story:runtime-metrics` (`depends_on`), which also edits `std/runtime.ignite` and `examples/todo/todo.ignite`.
