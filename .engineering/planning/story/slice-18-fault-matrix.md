---
format: aep.planning-md/3
id: story:slice-18-fault-matrix
kind: story
status: draft
title: The todo system passes the slice 18 fault matrix
relations:
- decomposes: epic:slice-18-one-service-operated
- depends_on: story:slice-18-contract
- depends_on: story:durable-todo-service
- depends_on: story:apply-recovery-way-out
scope:
- confidence: cited
  path: crates/ignite/tests/common/apply.rs
- confidence: cited
  path: crates/ignite/tests/main.rs
- confidence: cited
  path: crates/ignite/tests/slice18.rs
revision: 4
---
# Story: slice-18-fault-matrix

## Outcome

On the todo system, a host killed after an effect and before its receipt, a plan saved before a source change, and a journal with a truncated or altered last record each end in the stated recovery or refusal, on both targets.

## Context

- Unit U6 of `docs/slice.md` 22.13 (branch `feat/slice18-one-service`, contract at `19c70564`): owned files, wave and shared-file rules are there; decisions in 22.12.
- Depends on: story:durable-todo-service, story:apply-recovery-way-out.

## Acceptance

- The first failing test `slice18::llvm::todo_survives_sigkill_and_a_rollout` is red before and green after, on both targets where it has a target.
- Rows 16 to 22 of 22.0 pass with the exact results the table names; rows 8 and 31 hold; `task gate` green.

## Out of Scope

- Files 22.13 gives to another unit.
