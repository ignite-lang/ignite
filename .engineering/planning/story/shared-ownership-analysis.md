---
format: aep.planning-md/3
id: story:shared-ownership-analysis
kind: story
status: draft
title: One ownership analysis exported from ignite-types
scope:
- confidence: inferred
  path: crates/ignite-check/src/lib.rs
- confidence: inferred
  path: crates/ignite-llvm/src/region.rs
- confidence: inferred
  path: crates/ignite-rustgen/src/lib.rs
- confidence: inferred
  path: crates/ignite-types/src/moves.rs
revision: 5
---
# Story: shared-ownership-analysis

## Outcome

The set of mutating builtins and the in-place and copy decisions are computed once in `ignite-types` and read by both backends and the checker.

## Context

- Recommendation 2 of the architecture review (`docs/research/review-2026-11-05-architecture.md`), effort M.
- Evidence: the mutating-builtin set is written five times (`crates/ignite-types/src/moves.rs:499-517`, `crates/ignite-llvm/src/region.rs:24-28`, `crates/ignite-rustgen/src/lib.rs:5694`, `crates/ignite-check/src/lib.rs:2574`, `:3906-3913`), and they disagree on Store builtins and closure bodies.

## Acceptance

- `region::modified`, `region::root` and rustgen's `mutated_*` are gone; a test lists the one table; goldens unchanged except where the old sets disagreed, each named in the commit
- `task gate` green.

## Out of Scope

- The other recommendations of the review.
