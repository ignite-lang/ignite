---
format: aep.planning-md/3
id: story:slice-18-fault-matrix
kind: story
status: implemented
title: The todo system passes the slice 18 fault matrix
relations:
- decomposes: epic:slice-18-one-service-operated
- depends_on: story:slice-18-contract
- depends_on: story:durable-todo-service
- depends_on: story:apply-recovery-way-out
- depends_on: story:host-journal-integrity
scope:
- confidence: cited
  path: crates/ignite/tests/common/apply.rs
- confidence: cited
  path: crates/ignite/tests/main.rs
- confidence: cited
  path: crates/ignite/tests/slice18.rs
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-05T18:06:35Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"review_outcome":2}}}
- {from: "proposed", to: "active", at: "2026-11-05T18:06:35Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"review_outcome":2}}}
- {from: "active", to: "implemented", at: "2026-11-05T19:13:05Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"test_result":1,"review_outcome":2}}}
---
# Story: slice-18-fault-matrix

## Outcome

On the todo system, an `Availability` contract over the HTTP probe reaches each `Status` under
`ignite run` with a manual clock (row 16, both targets); and on the LLVM target a todo survives a
SIGKILL and a rollout, and a host killed after an effect and before its receipt, a plan saved
before a source change, and a journal with a torn, altered or cut last record each end in the
stated recovery or refusal (rows 17 to 22, LLVM only, 22.8). Unit U6 of `docs/slice.md` 22.13.

## Context

- Contract: 22.8 on branch `feat/slice18-one-service` (`19c70564`).
- Reads the repair record, the anchor and the `inspect` report of `story:host-journal-integrity`
  (rows 20 to 22), the todo of `story:durable-todo-service`, and `ignite recover` of
  `story:apply-recovery-way-out`.

## Acceptance

- `slice18::llvm::todo_survives_sigkill_and_a_rollout` is red before and green after.
- Row 16 (`slice18::{rust,llvm}::todo_contract_reaches_every_status`) and rows 17 to 22 pass with
  the exact results of 22.0; rows 8 and 31 hold; `task gate` green.

## Out of Scope

- Files 22.13 gives to another unit.
