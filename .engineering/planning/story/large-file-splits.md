---
format: aep.planning-md/3
id: story:large-file-splits
kind: story
status: draft
title: Split the largest compiler files into modules without changing public paths
relations:
- depends_on: story:mid-level-ir
scope:
- confidence: inferred
  path: crates/ignite-check/src/lib.rs
- confidence: inferred
  path: crates/ignite/src/apply/plan.rs
revision: 3
---
# Story: large-file-splits

## Outcome

`crates/ignite-check/src/lib.rs` (4,293 lines) and `crates/ignite/src/apply/plan.rs` (2,356) are split into private modules re-exported at the old paths; the backends follow after `story:mid-level-ir`.

## Context

- Recommendation 10 of the architecture review (`docs/research/review-2026-11-05-architecture.md`), effort S.
- Evidence: split plan in section 3 of the review; a rustgen split conflicts with `story:rustgen-generated-name-collisions` if run at the same time.

## Acceptance

- every `pub` path compiles unchanged; no golden changes; the two files are under 1,000 lines each
- `task gate` green.

## Out of Scope

- The other recommendations of the review.
