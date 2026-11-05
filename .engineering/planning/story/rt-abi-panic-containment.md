---
format: aep.planning-md/3
id: story:rt-abi-panic-containment
kind: story
status: implemented
title: A Rust panic in the LLVM runtime fails the task instead of aborting the process
scope:
- confidence: inferred
  path: crates/ignite-rt/src
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-05T19:10:27Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-11-05T19:10:27Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-11-05T19:44:33Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---
# Story: rt-abi-panic-containment

## Outcome

Every `extern "C"` entry of `ignite-rt` catches a Rust panic and turns it into an Ignite failure of the current task, with the report printed; store thunks are `Option<extern "C" fn>`.

## Context

- Recommendation 3 of the architecture review (`docs/research/review-2026-11-05-architecture.md`), effort S.
- Evidence: 91 `extern "C"` entries and no `catch_unwind` outside `crates/ignite-io/src/report.rs:174`; a panic unwinding into an `extern "C"` frame aborts (inferred behaviour); null thunks are UB (`crates/ignite-rt/src/store.rs:95-109`).

## Acceptance

- a test makes a runtime entry panic on a task thread of an LLVM program: the task fails with the report, the process and its other tasks keep running
- `task gate` green.

## Out of Scope

- The other recommendations of the review.
