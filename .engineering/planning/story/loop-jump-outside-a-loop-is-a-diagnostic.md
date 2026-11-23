---
format: aep.planning-md/3
id: story:loop-jump-outside-a-loop-is-a-diagnostic
kind: story
status: draft
title: '`break` or `continue` outside a loop is a front-end diagnostic'
revision: 2
---
# Story: `break` or `continue` outside a loop is a front-end diagnostic

## Outcome

`break` or `continue` with no enclosing loop in the same fn or closure body is refused by `ignite check`, in statement and expression form; a closure inside a loop does not make a jump in its body valid.

## Why

IG-18, severity medium, class logic; observed with a released command (the maintainer review of 2026-11-23 against main at a1d1407e, v0.16.4 archive).

An `Int` fn whose body is only `continue`, and the same with `break`, both passed `ignite check`; the Rust target then rejected the emitted jumps with E0268. A control with `loop { break }` passed.

The type checker marks the innermost loop if there is one and types the jump `Never` (`crates/ignite-types/src/typer.rs:2164` to `:2170` for statements, `:2367` to `:2372` for expressions); `continue` is typed `Never` without a check. The Rust generator emits `break;` and `continue;` as written (`crates/ignite-rustgen/src/lib.rs:4099`, `:4100`). No diagnostic for a jump outside a loop exists in the checker or the type checker at a1d1407e.

## Acceptance

- Fixtures, red first: statement `break`, statement `continue`, `None => break` and `None => continue` in a fn without a loop, and `break` in a closure body defined inside a loop: each is a diagnostic from `ignite check`, with its code listed in `docs/slice.md`.
- The programs of the adversary cases `break_or_continue_in_a_nested_while_condition_builds` (Rust E0590) and `break_outside_a_loop_is_refused_or_builds` (checks; LLVM panics at `lower.rs:694`; rustc E0268), in commit `65ea0106` of branch `wave/16-short-circuit`, file `crates/ignite/tests/short_circuit_adversary.rs` (removed from the tree in `9f38fb6e` so the count of ignored tests stays at its bar), come back as tests: each is a diagnostic from `ignite check` or builds and gives the evaluator's value on both targets.
- Controls: `loop { break }`, `continue` in a `while`, `None => break` inside a `for`; `task gate` green.

## Scope

- `crates/ignite-types/src/typer.rs:2165`, `:2367` (cited), `crates/ignite-rustgen/src/lib.rs:4099` (cited). Each read at a1d1407e, as the review states.
