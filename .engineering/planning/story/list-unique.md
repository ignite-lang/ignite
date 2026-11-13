---
format: aep.planning-md/3
id: story:list-unique
kind: story
status: draft
title: A List drops repeated elements and keeps the order
relations:
- informed_by: story:std-set
revision: 1
---
# Story: a List drops repeated elements and keeps the order

## Outcome

`xs.unique()` returns the elements of a list without repeats, each at the position of its first occurrence, on the evaluator and both targets.

## Why

Requested on 2026-11-12 by the author of a command-line tool written in Ignite, which wrote an order-keeping dedup by hand. A `Set` (`story:std-set`) answers membership and does not keep the order of first occurrence; `sort()` followed by a dedup of neighbours (`story:list-sort`) loses it too.

## Acceptance

- `xs.unique() -> List<T>` for every `T` that `==` accepts; another `T` is one diagnostic that names `unique_by`.
- `xs.unique_by(f)` with `f: fn(T) -> K` keeps the first element of each key.
- The cost is stated and held by a count: for a `T` that can be a `Map` key the instructions for 100,000 elements against 10,000 meet a ratio an O(n log n) or O(n) method meets and O(n²) does not; for other `T` the quadratic cost is stated in `docs/slice.md`.
- A property test on generated lists: the result has no repeats, is a subsequence of the input, and `xs.unique().unique() == xs.unique()`.
- The receiver is unchanged; the LLVM target's allocation count for 1,000 elements is written down.
- `docs/slice.md` and `site/book` list the methods; `task gate` green.

## Scope

- `crates/ignite-types/src/builtins.rs`, `crates/ignite-check/src/lib.rs`, `crates/ignite-rustgen/src/lib.rs`, `crates/ignite-llvm/`, the evaluator, `docs/slice.md`. Inferred.

## Not in this story

- A `Set` type (`story:std-set`); removal of repeated neighbours only.
