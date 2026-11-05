---
format: aep.planning-md/3
id: story:layering-general-names
kind: story
status: draft
title: The layering check covers std names in the general crates
scope:
- confidence: inferred
  path: crates/ignite-check/tests/layering.rs
- confidence: inferred
  path: crates/ignite/src/run/live.rs
- confidence: inferred
  path: docs/layering.md
revision: 4
---
# Story: layering-general-names

## Outcome

`layering::` refuses std module and type names in `crates/*/src` outside an allow-list, and the `std.sli.ProbeAttempt` fallback is gone.

## Context

- Recommendation 8 of the architecture review (`docs/research/review-2026-11-05-architecture.md`), effort S.
- Evidence: rule 5 catches protocol names only (`crates/ignite-check/tests/layering.rs:281`); the fallback is at `crates/ignite/src/run/live.rs:303-311`; `std.control.Machine` in `crates/ignite-types/src/typer/system.rs:1667-1672`. Slice 18 U1 touches `live.rs`; do this after it.

## Acceptance

- a new test in `layering.rs` fails on a std name outside the allow-list; the allow-list is in `docs/layering.md`
- `task gate` green.

## Out of Scope

- The other recommendations of the review.
