---
format: aep.planning-md/3
id: story:slice-16-moves-analysis
kind: story
status: active
title: Last-use moves analysis in ignite-types
summary: Compute last uses and consumed parameters for every typed module, read by no backend yet
relations:
- decomposes: epic:slice-16-language-gaps
- depends_on: story:slice-16-contract
scope:
- confidence: cited
  path: crates/ignite-types/src/lib.rs
- confidence: cited
  path: crates/ignite-types/src/moves.rs
- confidence: cited
  path: crates/ignite-types/tests/main.rs
- confidence: cited
  path: crates/ignite-types/tests/moves.rs
- confidence: cited
  path: crates/ignite/tests/fixtures/slice16/moves.ignite
- confidence: cited
  path: crates/ignite/tests/fixtures/slice16/moves_scale.ignite
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-03T23:35:37Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-11-03T23:35:37Z", actor: "human:timo", revision: 4}
---
# Story: slice-16-moves-analysis

## Outcome

`ignite-types` computes, for every typed module, which uses of a variable are its last use and
which parameters a function consumes on every path (`moves`, `consumed`), as `docs/slice.md` 20.3
defines them. No backend reads the result yet.

## Context

- Contract: `docs/slice.md` section 20 on `feat/slice16-language-gaps`, unit U1 of 20.10, rows 1
  to 5 of 20.0, decisions D1 to D3, the review findings B1 and B2 folded into 20.3.
- First failing test: `moves::a_var_passed_and_reassigned_moves` (ignite-types).

## Acceptance

- Rows 1 to 5 of 20.0 pass with the commands and results the table names.
- Rows 24 and 25 hold: no existing golden changes, `cargo test --workspace` green.

## Out of Scope

- Any backend change (U3, U4); rest patterns (U5).
