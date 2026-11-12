---
format: aep.planning-md/3
id: story:map-is-empty
kind: story
status: draft
title: A Map answers is_empty
relations:
- informed_by: story:map-values
revision: 1
---
# Story: a Map answers is_empty

## Outcome

`m.is_empty()` is a builtin method of `Map<K, V>`, as it is of `List` and `Text`, on the evaluator and both targets.

## Why

Requested on 2026-11-13 by the author of a command-line tool written in Ignite. `List` and `Text` have `len` and `is_empty`; `Map` has `len` only (`crates/ignite-types/src/builtins.rs`), so the same question is written `m.len() == 0` for one of the three collection types.

## Acceptance

- `m.is_empty() -> Bool` for every `Map<K, V>`; true for a new map and after the last key is removed.
- One differential program gives identical output on the evaluator, the Rust target and the LLVM target.
- The method reads the map: it allocates nothing on either target (a count test), and the receiver may be a lent parameter.
- `docs/slice.md` and `site/book` list the method; `task gate` green.

## Scope

- `crates/ignite-types/src/builtins.rs`, `crates/ignite-rustgen/src/lib.rs`, `crates/ignite-llvm/`, the evaluator, `docs/slice.md`. Inferred.

## Not in this story

- `values`, `update` and `upsert` (`story:map-values`, `story:map-update-and-upsert`).
