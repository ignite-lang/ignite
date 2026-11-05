---
format: aep.planning-md/3
id: story:apply-cycle-timing
kind: story
status: draft
title: ignite-perf times the edit-build-restart cycle under ignite apply
relations:
- decomposes: epic:slice-18-one-service-operated
- depends_on: story:slice-18-contract
- depends_on: story:durable-todo-service
scope:
- confidence: cited
  path: tools/ignite-perf/src
revision: 2
---
# Story: apply-cycle-timing

## Outcome

`ignite-perf` has a mode `apply-cycle` that times the edit-build-restart cycle under `ignite apply` (22.15; the terminal design before and after, the todo after only, D12).

## Context

- Unit U7 of `docs/slice.md` 22.13 (branch `feat/slice18-one-service`, contract at `19c70564`): owned files, wave and shared-file rules are there; decisions in 22.12.
- Depends on: story:durable-todo-service.

## Acceptance

- The first failing test `ignite-perf self-check naming apply-cycle` is red before and green after, on both targets where it has a target.
- Rows 28 (mode) of 22.0 pass with the exact results the table names; rows 8 and 31 hold; `task gate` green.

## Out of Scope

- Files 22.13 gives to another unit.
