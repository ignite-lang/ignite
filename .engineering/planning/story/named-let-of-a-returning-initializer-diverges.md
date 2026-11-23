---
format: aep.planning-md/3
id: story:named-let-of-a-returning-initializer-diverges
kind: story
status: draft
title: A named binding keeps the fact that its initializer returns
revision: 1
---
# Story: a named binding keeps the fact that its initializer returns

## Outcome

An `Int` fn ending in `let result = return 7` checks, as the same fn ending in `let _ = return 7` does.

## Why

IG-20, severity medium, class logic; observed with a released command (the maintainer review of 2026-11-23 against main at a1d1407e, v0.16.4 archive).

The named form failed with E040 because the block supposedly produced no value; the `_` form passed.

The pattern-binding branch of `let` types the statement `Never` when the initializer is `Never` (`crates/ignite-types/src/typer.rs:2080` to `:2085`). The named-binding branch binds and returns `Unit` unconditionally (`:2112`, `:2113`). The block counts a statement as diverging only when it is typed `Never` (`:2036`, `:2037`).

## Acceptance

- A test, red first: `let result = return 7` and `let r: Int = return 7` at the end of an `Int` fn check and run on both targets.
- Controls: `let _ = return 7` still checks; `let x = 7` at the end of an `Int` fn stays E040; `task gate` green.

## Scope

- `crates/ignite-types/src/typer.rs:2081`, `:2113`, `:2036` (cited). Each read at a1d1407e, as the review states.
