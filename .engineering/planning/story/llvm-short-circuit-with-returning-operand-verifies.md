---
format: aep.planning-md/3
id: story:llvm-short-circuit-with-returning-operand-verifies
kind: story
status: active
title: '`&&` and `||` whose right operand returns produce valid LLVM IR'
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T03:51:03Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-23T03:51:04Z", actor: "human:timo", revision: 3}
---
# Story: `&&` and `||` whose right operand returns produce valid LLVM IR

## Outcome

`fn keep(p: Bool) -> Bool { p && (return true) }` and the `||` form build on the LLVM target and give the values the Rust target gives, for both inputs.

## Why

IG-07, severity high, class logic; observed with a released command and the LLVM verifier (the maintainer review of 2026-11-23 against main at a1d1407e, v0.16.4 archive).

The function above passed `ignite check`; the Rust target gave the expected values for both inputs; the LLVM verifier rejected the emitted program because a phi node listed a predecessor that had already returned.

`binary_step` lowers the right operand (`crates/ignite-llvm/src/lower.rs:3422`), takes the current block as `rhs_end` (`:3424`) and always adds `[ r, %rhs_end ]` to the phi (`:3429`). `place` (`crates/ignite-llvm/src/ir.rs:975` to `:983`) adds a fall-through branch only when the block is not terminated, so after a `return` the merge block has no edge from `rhs_end`.

## Acceptance

- A native test on both targets, red first on LLVM: `&&` and `||` with a right operand that returns, and with one that falls through, each for both inputs.
- Controls: the existing short-circuit tests stay green; `task gate` green.

## Scope

- `crates/ignite-llvm/src/lower.rs:3409` to `:3431` (`binary_step`, cited `:3422`, `:3429`), `crates/ignite-llvm/src/ir.rs:975` (`place`). Each read at a1d1407e, as the review states.
