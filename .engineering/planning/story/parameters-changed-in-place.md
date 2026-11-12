---
format: aep.planning-md/3
id: story:parameters-changed-in-place
kind: story
status: draft
title: A fn changes a caller's variable through a parameter
revision: 1
---
# Story: a fn changes a caller's variable through a parameter

## Outcome

A fn declares that it changes one of its arguments, the caller passes a `var` place, and after the call the place holds the changed value, without the caller writing the result back, on the evaluator and both targets.

## Why

Requested on 2026-11-12 by the author of a command-line tool written in Ignite: a changed parameter is the callee's copy (`docs/slice.md`, the value semantics of parameters), so a fn that advances a state returns `(State, T)` and every caller reassigns. The form is frequent in parsers and in anything that threads a cursor.

## Acceptance

- A design step first, in `docs/`, with at least these options and a decision: a marked parameter and a marked argument (`fn next(inout s: State) -> T`, `next(&s)` or the like); sugar over the tuple return that the checker expands; nothing (the tuple return stays the only form) with the reason. For each: what a reader sees at the call, what happens when two arguments name one place, and what the move analysis (`crates/ignite-types/src/moves.rs`) must add.
- Value semantics hold: no value is observed changed through another name. The design states the rule that refuses an aliased call and its diagnostic.
- If a form is chosen: one differential program (a cursor over `Bytes`, a counter in a record, an element of a list) on the evaluator and both targets; the Rust target passes `&mut` and a count test shows no copy of the state per call; the LLVM target's count is written down.
- `docs/slice.md` states the rule; `task gate` green.

## Scope

- `docs/` (design note), then `crates/ignite-syntax/`, `crates/ignite-types/src/typer.rs`, `moves.rs`, `crates/ignite-rustgen/src/lib.rs`, `crates/ignite-llvm/`, the evaluator. Inferred.

## Not in this story

- References as values (a reference stored in a record or returned).
