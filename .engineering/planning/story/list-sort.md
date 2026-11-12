---
format: aep.planning-md/3
id: story:list-sort
kind: story
status: draft
title: A List is sorted with sort and sort_by
revision: 1
---
# Story: a List is sorted with sort and sort_by

## Outcome

A program orders a `List<T>` with `sort()` when `T` has an order (`Int`, `Float`, `Text`, `Bytes`, `Byte`, `Bool`, a payload-free enum, a tuple of those) and with `sort_by(key)` or a comparison fn for any other `T`, on the evaluator and both targets, with the same result everywhere.

## Why

Requested on 2026-11-12 by the author of a command-line tool written in Ignite (a report generator that collects records from two HTTPS APIs): at v0.16.2 neither the builtin methods of `List` nor `std` have a sort. `Map` iterates in key order, which covers only the case where the sort key is a unique key type.

## Acceptance

- `xs.sort()` returns the elements in ascending order for every type listed in the outcome; a type without an order is refused by `ignite check` with one diagnostic that names `sort_by`.
- `xs.sort_by(f)` with `f: fn(T) -> K` orders by the key `K`; the sort is stable (a test with equal keys pins the original order).
- `Text` and `Bytes` order by bytes; `Float` orders as `<` does, and where NaN lands is stated in `docs/slice.md` and pinned by a test.
- A list of 100,000 `Int` in reverse order sorts in O(n log n): an instruction count test (callgrind) bounds it against the count for 10,000.
- The evaluator, the Rust target and the LLVM target give identical output for one differential program; the LLVM target's allocation count for a sort of 1,000 elements is written down.
- `docs/slice.md` states the forms; `site/book` lists them; `task gate` green.

## Scope

- `crates/ignite-types/src/typer.rs` (builtin methods of `List`), `crates/ignite-rustgen/src/lib.rs`, `crates/ignite-llvm/`, the evaluator, `docs/slice.md`. Inferred.

## Not in this story

- A sorted collection type or a `Set` (`story:std-set`).
