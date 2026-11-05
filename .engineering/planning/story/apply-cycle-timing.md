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
revision: 3
---
# Story: apply-cycle-timing

## Outcome

`tools/ignite-perf` measures what rows 27 and 28 of `docs/slice.md` 22.0 need: a new mode
`apply-cycle` times the edit-build-restart cycle under `ignite apply` (22.15), and mode `programs`
times `todod` and `health` as separate units. Unit U7 of 22.13.

## Context

- Contract: 22.15 on branch `feat/slice18-one-service` (`19c70564`); D12 (the todo after only).
- `programs` today times the whole `examples/todo/todo.ignite` build as one unit
  (`tools/ignite-perf/src/perf.rs:1775`); row 27 needs `todod` alone.
- U8 (`story:slice-18-records`) runs both modes and writes the records.

## Acceptance

- Before: `ignite-perf apply-cycle` is refused as an unknown mode. After: run on one tree it
  writes, per target, 5 samples after one discarded warm-up for `apply-cycle-terminal-<target>`
  and `apply-cycle-todo-<target>`, each sample split into plan (build included), apply until
  `completed`, and first answer; `ignite-perf self-check` exits 0 with a case for the mode.
- After: `ignite-perf programs` writes separate units for `todod` and `health` per target and
  profile.
- Rows 8 and 31 hold; `task gate` green.

## Out of Scope

- The records themselves (U8).
