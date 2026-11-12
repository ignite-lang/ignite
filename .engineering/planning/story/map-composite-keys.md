---
format: aep.planning-md/3
id: story:map-composite-keys
kind: story
status: draft
title: A Map key may be a tuple or a record of key types
revision: 1
---
# Story: a Map key may be a tuple or a record of key types

## Outcome

`Map<K, V>` accepts a tuple, a record or an enum with payloads as `K` when every part is itself a key type, ordered part by part, on the evaluator and both targets.

## Why

Requested on 2026-11-12 by the author of a command-line tool written in Ignite. `docs/slice.md` O11 limits keys to scalars, payload-free enums and `Ref`; any other key type is E106. Grouping by two values (a name and a version) needs one joined `Text` key today.

## Acceptance

- `Map<(Text, Int), V>`, `Map<R, V>` for a record `R` of key types, and `Map<E, V>` for an enum `E` whose payloads are key types compile and behave as a model built from a joined `Text` key on generated inputs.
- Order: tuples and records compare part by part in declaration order; enum variants in declaration order, then payloads. `for (k, v) in m` visits in that order on the evaluator and both targets (one differential program).
- A key type with a fn value, a `List`, a `Map` or a handle in it stays E106, with the part named in the diagnostic.
- The LLVM target's sorted array holds such keys with their copy and drop glue; a leak test (`docs/leak-guard.md`) covers insert, replace and remove of a key that owns a `Text`.
- Allocations per `put` and per `get` for a `(Text, Int)` key are counted on both targets and written down.
- `docs/slice.md` O11 and the `Map` section state the rule; `task gate` green.

## Scope

- `crates/ignite-types/src/typer.rs` (E106), `crates/ignite-rustgen/src/lib.rs` (derives for `Ord`), `crates/ignite-llvm/` (key compare, copy, drop), the evaluator, `docs/slice.md`. Inferred.

## Not in this story

- A `List` or `Bytes` as a key part; hashing maps.
