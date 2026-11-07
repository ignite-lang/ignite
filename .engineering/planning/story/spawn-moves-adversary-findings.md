---
format: aep.planning-md/3
id: story:spawn-moves-adversary-findings
kind: story
status: draft
title: Spawned ends close in branches on LLVM, named spawn args in order on Rust
revision: 1
---
# Story: spawn-moves-adversary-findings

## Outcome

A pipe end spawned in a branch or before a shadowing binding closes on task failure on both targets, the spawn moves of PR 77 have tests that kill their mutants, and a spawn with named arguments out of parameter order builds on Rust.

## Context

- Adversary review of PR 77 (2026-11-07); branch `test/adv-spawn-moves` at `5234c0f1` adds `moves::` cases and the fixtures `streams/nested_spawn.ignite`, `streams/named_spawn.ignite` with two `stream_ownership::` tests.
- F1 (introduced, red): a spawn in an `if` branch or before a binding that shadows the end's name closes the end on Rust since PR 77, but LLVM keeps it open until the body ends (`crates/ignite-llvm/src/lower.rs:5036` hands an end over only in its own block, by unshadowed name); `docs/slice.md` 20.3.1 (`:6734`) claims both targets.
- F2, F3 (introduced): mutants `receiver_taken = false` (`crates/ignite-types/src/moves.rs:1565`) and dropping `Operand::Spawned` from the held operands (`:1584`) survived the old suite; the branch's two `moves::` cases kill them.
- F4 (pre-existing, red): named spawn arguments out of parameter order are emitted in source order on Rust, E0308 (`crates/ignite-rustgen/src/lib.rs:3975`).

## Acceptance

- The tests of `test/adv-spawn-moves` are on main and pass: `stream_ownership::a_spawn_in_a_branch_or_before_a_shadowing_binding_closes_the_end_on_both_targets` and `named_spawn_arguments_in_another_order_build_and_pass_on_both_targets` on both targets; the two `moves::` cases.
- `CODEGEN_REVISION` bumped; `task gate` green.
