---
format: aep.planning-md/3
id: story:slice-16-llvm-moves
kind: story
status: implemented
title: LLVM target moves at last use
summary: Own a parameter iff consumed, move places at last use and lower expression calls on the LLVM target
relations:
- decomposes: epic:slice-16-language-gaps
- depends_on: story:slice-16-moves-analysis
- depends_on: story:slice-16-expression-calls
scope:
- confidence: cited
  path: crates/ignite-llvm/src
- confidence: cited
  path: crates/ignite-llvm/tests
- confidence: cited
  path: crates/ignite/tests/slice16.rs
- confidence: cited
  path: docs/llvm.md
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-04T07:03:30Z", actor: "human:timo", revision: 5}
- {from: "proposed", to: "active", at: "2026-11-04T07:03:30Z", actor: "human:timo", revision: 6}
- {from: "active", to: "implemented", at: "2026-11-05T00:57:41Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"test_result":1}}}
---
# Story: slice-16-llvm-moves

## Outcome

The LLVM target owns a parameter if and only if it is consumed, moves places at their last use
and lowers expression calls where `calls.ignite` needs it (20.11), with the same results as the Rust target.

## Context

- Contract: `docs/slice.md` section 20 on `feat/slice16-language-gaps`, unit U4 of 20.10, rows 10,
  12 and 14 of 20.0; reads `moves` and `consumed` from U1 and the call typing of U2.
- First failing test: `golden::last_use_moves_and_expression_calls` (ignite-llvm).
- Field targets from the playground session (2026-11-04, callgrind of the ember server on LLVM,
  after round 11): `List<Byte>` copies of `entries` 144,030, the argument list 34,018,
  `parse_array` pushes 22,010. Which of them a last-use move removes is not measured.

## Acceptance

- Rows 10, 12 and 14 of 20.0 pass; rows 24 to 26 hold.
- `docs/llvm.md` §3's borrowed/owned sentence names the consumed rule (a parameter is owned iff it
  is consumed) and §4's soundness sentence ("Callee arguments are fresh copies or the caller's
  places") names the moved place, as 20.3.5 requires.
- Appends rows 10 and 12 to `crates/ignite/tests/slice16.rs`, which U2 creates; it does not edit
  `CODEGEN_REVISION` (U3 bumps it for both, row 15).

## Out of Scope

- The Rust target (U3).
