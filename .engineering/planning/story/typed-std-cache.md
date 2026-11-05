---
format: aep.planning-md/3
id: story:typed-std-cache
kind: story
status: draft
title: Typed std modules are cached between CLI runs
relations:
- depends_on: story:stable-node-ids
scope:
- confidence: inferred
  path: crates/ignite-types/src
- confidence: inferred
  path: crates/ignite/src/apply
- confidence: inferred
  path: crates/ignite/src/lib.rs
revision: 4
---
# Story: typed-std-cache

## Outcome

`ignite check`, `plan`, `run` and `apply` reuse the typed std modules of an unchanged std instead of parsing, checking and typing them each time; one apply operation loads its program once.

## Context

- Recommendation 5 of the architecture review (`docs/research/review-2026-11-05-architecture.md`), effort M.
- Evidence: nothing is cached per CLI spawn (`crates/ignite/src/lib.rs:585-647`, `:822-832`); one apply loads the program four times (`apply/plan.rs:1171`, `:1207`, `apply/revision.rs:25`, `host/controller.rs:450`); `docs/friction.md` row 43 (todo check 0.068 to 0.131 s). Needs `story:stable-node-ids`.

## Acceptance

- `ignite check examples/todo/todo.ignite` warm is at most the 0.068 s of the slice 16 tree, measured in 15 pairs
- `task gate` green.

## Out of Scope

- The other recommendations of the review.
