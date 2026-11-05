---
format: aep.planning-md/3
id: story:test-timing-hygiene
kind: story
status: active
title: Tests do not depend on wall-clock bounds under load
scope:
- confidence: inferred
  path: crates/ignite-rt/tests
- confidence: inferred
  path: crates/ignite-types/tests
- confidence: inferred
  path: crates/ignite/tests
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-05T20:33:19Z", actor: "human:timo", revision: 5}
- {from: "proposed", to: "active", at: "2026-11-05T20:33:19Z", actor: "human:timo", revision: 6}
---
# Story: test-timing-hygiene

## Outcome

Wall-bounded assertions use lower bounds, a manual clock or a deadline, and the 50 ms race in the pipe tests is gone.

## Context

- Recommendation 9 of the architecture review (`docs/research/review-2026-11-05-architecture.md`), effort S.
- Evidence: `crates/ignite/tests/run.rs:684-687`, `crates/ignite-types/tests/limits.rs:44`, `:65`, `:109` (failed under load in slice 15), `crates/ignite-rt/tests/pipe.rs:228-233`; the known gate flakes in `AGENTS.md`.

## Acceptance

- each named assertion has no upper wall-clock bound below 10 s or runs on the manual clock; 3 gates at load over 20 pass
- `task gate` green.

## Out of Scope

- The other recommendations of the review.
