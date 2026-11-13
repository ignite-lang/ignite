---
format: aep.planning-md/3
id: story:list-sort
kind: story
status: implemented
title: A List is sorted with sort and sort_by
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-12T20:26:59Z", actor: "human:timo", revision: 2, executor: "agent:claude-coordinator", correlation: "accepted-2026-11-12"}
- {from: "proposed", to: "active", at: "2026-11-12T20:26:59Z", actor: "human:timo", revision: 3, executor: "agent:claude-coordinator", correlation: "accepted-2026-11-12"}
- {from: "active", to: "implemented", at: "2026-11-13T07:29:34Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"test_result":2,"review_outcome":1}}, executor: "agent:claude-coordinator", correlation: "wave-15"}
---
# Story: a List is sorted with sort and sort_by

## Outcome

A program orders a `List<T>` with `sort()` when `T` has an order (`Int`, `Text`, `Bytes`, `Byte`, `Bool`, a payload-free enum, a tuple of those) and with `sort_by(key)` or a comparison fn for any other `T`, on the evaluator and both targets, with the same result everywhere.

## Why

Requested on 2026-11-12 by the author of a command-line tool written in Ignite (a report generator that collects records from two HTTPS APIs): at v0.16.2 neither the builtin methods of `List` nor `std` have a sort. `Map` iterates in key order, which covers only the case where the sort key is a unique key type.

## Acceptance

- `xs.sort()` returns the elements in ascending order for every type listed in the outcome; a type without an order is refused by `ignite check` with one diagnostic that names `sort_by`.
- `xs.sort_by(f)` with `f: fn(T) -> K` orders by the key `K`; the sort is stable (a test with equal keys pins the original order).
- `Text` and `Bytes` order by bytes.
- A list of 100,000 `Int` in reverse order sorts in O(n log n): an instruction count test (callgrind) bounds it against the count for 10,000.
- The evaluator, the Rust target and the LLVM target give identical output for one differential program; the LLVM target's allocation count for a sort of 1,000 elements is written down.
- `docs/slice.md` states the forms; `site/book` lists them; `task gate` green.

## Scope

- `crates/ignite-types/src/typer.rs` (builtin methods of `List`), `crates/ignite-rustgen/src/lib.rs`, `crates/ignite-llvm/`, the evaluator, `docs/slice.md`. Inferred.

## Not in this story

- A sorted collection type or a `Set` (`story:std-set`).

## Corrections

- 2026-11-13, by the coordinator while the unit was in work: revisions 1 to 3 listed `Float` among the ordered types and asked for a NaN rule pinned by a test. The language has no floating-point type (`docs/slice.md` says so, and `crates/ignite-types` declares none), so neither can be met or tested. Both are removed; nothing else changed. A floating-point type that arrives later states its own order in its own story.
