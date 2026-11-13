---
format: aep.planning-md/3
id: story:int-min-max-clamp
kind: story
status: draft
title: Int has min, max and clamp
revision: 1
---
# Story: Int has min, max and clamp

## Outcome

`a.min(b)`, `a.max(b)` and `x.clamp(lo, hi)` exist on `Int`, on the evaluator and both targets.

## Why

Requested on 2026-11-12 by the author of a command-line tool written in Ignite, which writes them as `if` expressions. std has the same need: `std/json.ignite` defines `fn smaller(a: Int, b: Int) -> Int`.

## Acceptance

- `a.min(b)`, `a.max(b)` on `Int`; `x.clamp(lo, hi)` with `lo <= hi`, and what `lo > hi` gives is stated and pinned by a test.
- The same three on `Float` with the NaN rule stated, or `Float` is named as not covered, with the reason.
- The Rust target emits the standard library's functions; the LLVM target emits no call (the emitted IR of one function is pinned by a test or a count of instructions).
- `std/json.ignite`'s `smaller` is replaced by `min`.
- `docs/slice.md` and `site/book` list the methods; `task gate` green.

## Scope

- `crates/ignite-types/src/builtins.rs`, `crates/ignite-check/src/lib.rs`, `crates/ignite-rustgen/src/lib.rs`, `crates/ignite-llvm/`, the evaluator, `std/json.ignite`, `docs/slice.md`. Inferred.

## Not in this story

- `abs`, `pow` and other arithmetic functions.
