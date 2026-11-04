---
format: aep.planning-md/3
id: story:slice-16-rust-moves
kind: story
status: draft
title: Rust target moves at last use
summary: Pass consumed parameters by value and move at last use on the Rust target, and lower expression calls
relations:
- decomposes: epic:slice-16-language-gaps
- depends_on: story:slice-16-moves-analysis
- depends_on: story:slice-16-expression-calls
scope:
- confidence: cited
  path: crates/ignite-rustgen/src/lib.rs
- confidence: cited
  path: crates/ignite-rustgen/tests
- confidence: cited
  path: crates/ignite/src/apply/plan.rs
- confidence: cited
  path: docs/slice.md
revision: 5
---
# Story: slice-16-rust-moves

## Outcome

The Rust target moves a value at its last use and passes consumed parameters by value, so the
quadratic copy of probe p07 becomes linear; expression calls are lowered where `calls.ignite` needs it (20.11).

## Context

- Contract: `docs/slice.md` section 20 on `feat/slice16-language-gaps`, unit U3 of 20.10, rows 9,
  11, 13 and 15 of 20.0; reads `moves` and `consumed` from U1 and the call typing of U2.
- First failing test: `golden::last_use_moves_and_expression_calls` (ignite-rustgen).
- Bumps `CODEGEN_REVISION` from the value at the wave's base (20.14 Q2).
- Ordering: `fix/rustgen-builtin-shadowing` edits the same `crates/ignite-rustgen/src/lib.rs`,
  `tests/golden.rs` and `CODEGEN_REVISION`. It lands on `main` first (release v0.4.1), and the
  slice branch merges `main` before W3 starts, so this unit starts from its result.
- Also edits section 6 of `docs/slice.md` (the parameter sentence and the `fn` row).
- Field targets from the playground session's round 9 (2026-11-04, tree `ignite-opt9-io-copies`,
  base `88cae227`), whose ad-hoc last-use moves in rustgen were not landed so that the compiler
  keeps the one move rule of 20.3: `write_all(&stream, out.clone())` becomes `out` at its last
  use, and the other move sites of that diff (`last_use`, `owned_args`). Its whole round, moves
  and borrows, took ember's `.clone()` count from 148 to 119 and Rust-target heap allocations
  from 564,409 to 514,375; `Parsed::Command(args.clone(), ..)` and
  `next.entries.push(entry.clone())` are 4.3 % and 3.2 % of allocations
  (`story:rustgen-redundant-clones`).

## Acceptance

- Rows 9, 11, 13 and 15 of 20.0 pass; rows 24 to 26 hold.

## Out of Scope

- The LLVM target (U4).
