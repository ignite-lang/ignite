---
format: aep.planning-md/3
id: story:slice-17-frontend
kind: story
status: implemented
title: 'Slice 17 frontend: the four handle types'
summary: Checker, typer, evaluator and lowering for Directory, Launcher, Entropy and Setting
relations:
- decomposes: epic:playground-substrate
- depends_on: story:slice-17-contract
- depends_on: story:slice-16-expression-calls
scope:
- confidence: cited
  path: crates/ignite-check/src/lib.rs
- confidence: cited
  path: crates/ignite-check/tests/layering.rs
- confidence: cited
  path: crates/ignite-eval/src
- confidence: cited
  path: crates/ignite-llvm/src
- confidence: cited
  path: crates/ignite-rustgen/src
- confidence: cited
  path: crates/ignite-types/src
- confidence: cited
  path: docs/layering.md
- confidence: cited
  path: std/fs.ignite
- confidence: cited
  path: std/process.ignite
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-05T03:52:11Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-11-05T03:52:11Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-11-05T05:10:38Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---
# Story: slice-17-frontend

## Outcome

The checker, typer, evaluator and both backends know the four handle types `Directory`,
`Launcher` (with `ChildProcess`), `Entropy` and `Setting`, their effects, their resource kinds and
their lowering, so the substrate units can fill in behaviour behind a stable surface.

## Context

- Contract: `docs/slice.md` section 21 on `feat/slice17-processes-files`, unit U1 of 21.12, and
  the frontend rows of 21.0 that 21.12 gives U1, among them row 35 (a parameter named `clock`).
- Owns `std/fs.ignite`, `std/process.ignite`, `SUBSTRATE_MODULES`, `docs/layering.md` and the
  lowering in `crates/ignite-rustgen` and `crates/ignite-llvm` as 21.12 lists.
- Ordering: edits `crates/ignite-check/src/lib.rs` and `crates/ignite-types/src/typer.rs`, which
  slice 16's W2 (`story:slice-16-expression-calls`) also edits; this unit starts after slice 16's
  W2 merges, on a base that holds it.

## Acceptance

- The rows of 21.0 that 21.12 assigns to U1 pass; rows 30 and 33 hold.

## Out of Scope

- The substrate behaviour of each handle (U2 to U5).
