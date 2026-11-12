---
format: aep.planning-md/3
id: story:map-values
kind: story
status: draft
title: A Map has values and entries
relations:
- informed_by: story:list-contains-and-filter
revision: 1
---
# Story: a Map has values and entries

## Outcome

A `Map<K, V>` answers `values()` and `entries()` as builtin methods, in key order, on the evaluator and both targets, with the same result everywhere.

## Why

Requested on 2026-11-12 by the author of a command-line tool written in Ignite, found while it built an HTTP server on `std.protocol.http`: the values of a map are collected by iterating `keys()` and calling `get` for each key. The builtin methods of `Map` (`crates/ignite-types/src/builtins.rs` `map_method`) are `get`, `put`, `remove`, `keys`, `len` and `contains`. `for (k, v) in m` iterates in key order (`docs/slice.md` section 14.3) and is the form available today; it gives no list to pass on.

## Acceptance

- `m.values() -> List<V>` gives the values in the order `keys()` gives their keys; `m.entries() -> List<(K, V)>` gives the pairs in that order. On an empty map both are empty lists.
- A property test on generated maps: `m.values()` equals `m.keys()` mapped through `get`, and `m.entries()` equals the pairs `for (k, v) in m` yields.
- The map is unchanged by both; a change to the returned list does not change the map.
- Both have the effect roots of `m`, as `m.get(k)` and `m.keys()` have (`docs/slice.md`, the rule on the container reads of a receiver of declared type `Map`): a `requires []` fn cannot use a `Setting` held in a map through them.
- Allocations on the Rust target: one list and one copy per value for `values()`, one list and one copy per key and per value for `entries()`, counted at two sizes; the LLVM target's counts are written down.
- `docs/slice.md` and `site/book` list the methods; `task gate` green.

## Scope

- `crates/ignite-types/src/builtins.rs`, `crates/ignite-check/src/lib.rs` (the name list), `crates/ignite-rustgen/src/lib.rs`, `crates/ignite-llvm/`, the evaluator, `docs/slice.md`. Inferred.

## Not in this story

- A `Set` (`story:std-set`), `Map.from(pairs)`, a `map` or `filter` over a map's entries.
- A borrowed iteration of values without a list: `for (k, v) in m` already is one.
