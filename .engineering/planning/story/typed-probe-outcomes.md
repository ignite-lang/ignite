---
format: aep.planning-md/3
id: story:typed-probe-outcomes
kind: story
status: draft
title: A contract's good is a typed predicate over its probe's outcome
relations:
- decomposes: epic:slice-18-one-service-operated
- depends_on: story:slice-18-contract
scope:
- confidence: cited
  path: crates/ignite-eval/src/model.rs
- confidence: cited
  path: crates/ignite-eval/src/validate.rs
- confidence: cited
  path: crates/ignite-types/src/typer/system.rs
- confidence: cited
  path: crates/ignite/src/apply/plan.rs
- confidence: cited
  path: crates/ignite/src/apply/revision.rs
- confidence: cited
  path: crates/ignite/src/lib.rs
- confidence: cited
  path: crates/ignite/src/run
- confidence: cited
  path: crates/ignite/tests/golden
- confidence: cited
  path: crates/ignite/tests/plan.rs
- confidence: cited
  path: crates/ignite/tests/run.rs
- confidence: cited
  path: examples/terminal/terminal.design.ignite
- confidence: cited
  path: std/contract.ignite
- confidence: cited
  path: std/control.ignite
- confidence: cited
  path: std/sli.ignite
- confidence: cited
  path: tools/ignite-perf/src/readiness.rs
revision: 16
---
# Story: typed-probe-outcomes

## Outcome

A probe kind declared outside `std/` names its outcome type and a contract's `good` is a `fn(T) -> Bool` over it, checked at compile time; the Telnet outcomes keep working as two named predicates; run journals move to `ignite-journal/0.2` (D1 to D3, D14).

## Context

- Unit U1 of `docs/slice.md` 22.13 (branch `feat/slice18-one-service`, contract at `19c70564`): owned files, wave and shared-file rules are there; decisions in 22.12.
- Depends on: story:slice-18-contract.

## Acceptance

- The first failing test `the prompt outcomes are predicates (std/sli.ignite)` is red before and green after, on both targets where it has a target.
- Rows 1 to 7, 29, 30, 34 of 22.0 pass with the exact results the table names; rows 8 and 31 hold; `task gate` green.

## Out of Scope

- Files 22.13 gives to another unit.
