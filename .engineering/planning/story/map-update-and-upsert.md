---
format: aep.planning-md/3
id: story:map-update-and-upsert
kind: story
status: draft
title: A Map entry is changed in one call
relations:
- informed_by: story:map-values
revision: 1
---
# Story: a Map entry is changed in one call

## Outcome

A `Map<K, V>` answers `update(k, f)` and `upsert(k, default, f)`, so a counter, an append to a list held in a map and a put-if-absent are one call each, on the evaluator and both targets.

## Why

Requested on 2026-11-12 by the author of a command-line tool written in Ignite: read-modify-write on a map is a frequent form in it, written `m.put(k, m.get(k).unwrap_or(d) + 1)` or as `get`, a change of a local and `put`. The builtin methods of `Map` (`crates/ignite-types/src/builtins.rs` `map_method`) are `get`, `put`, `remove`, `keys`, `len` and `contains`. A `Store` already has `update` and `upsert` (`store_method`); the names and argument order follow it.

## Acceptance

- `m.update(k, f)` with `f: fn(V) -> V` replaces the value of an existing key and returns `true`; on a missing key the map is unchanged and it returns `false`.
- `m.upsert(k, default, f)` applies `f` to the value of `k`, or to `default` when `k` is missing, and stores the result.
- `f` is called once per call; the key is evaluated once.
- Both are in-place changes of a `var` map: on a map that is not a place they give the diagnostic `put` gives there, and they are rows of `ignite_check::IN_PLACE`.
- A property test on generated maps and keys: `upsert(k, d, f)` equals `put(k, f(get(k).unwrap_or(d)))`.
- Allocations on the Rust target: `upsert` of a `List` value that pushes one element does not copy the list (counted at two list sizes); the LLVM target's counts are written down.
- `docs/slice.md` and `site/book` list the methods; `task gate` green.

## Scope

- `crates/ignite-types/src/builtins.rs`, `crates/ignite-check/src/lib.rs`, `crates/ignite-rustgen/src/lib.rs`, `crates/ignite-llvm/`, the evaluator, `docs/slice.md`. Inferred.

## Not in this story

- `values()` and `entries()` (`story:map-values`), composite keys (`story:map-composite-keys`).
