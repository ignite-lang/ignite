---
format: aep.planning-md/3
id: story:stable-node-ids
kind: story
status: draft
title: TypedProgram keyed by stable node ids instead of heap addresses
scope:
- confidence: inferred
  path: crates/ignite-syntax/src
- confidence: inferred
  path: crates/ignite-types/src
revision: 3
---
# Story: stable-node-ids

## Outcome

The typer's side tables are keyed by stable node ids and spans carry a file id, so typed modules can be cached, serialised and desugared.

## Context

- Recommendation 4 of the architecture review (`docs/research/review-2026-11-05-architecture.md`), effort M.
- Evidence: 18 tables keyed by `expr as *const Expr as usize` (`crates/ignite-types/src/lib.rs:1319-1377`); `Span` has no file id (`crates/ignite-syntax/src/ast.rs:6-9`).

## Acceptance

- no table in `crates/ignite-types/src/lib.rs` is keyed by an address; a test clones a `Program` and types the clone with the same results
- `task gate` green.

## Out of Scope

- The other recommendations of the review.
