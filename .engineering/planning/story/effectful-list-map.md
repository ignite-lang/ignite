---
format: aep.planning-md/3
id: story:effectful-list-map
kind: story
status: draft
title: List.map over effectful fns
summary: Let List.map and its siblings take a fn with the caller's effects, closing ember's E5
scope:
- confidence: inferred
  path: crates/ignite-check/src
- confidence: inferred
  path: crates/ignite-types/src
- confidence: inferred
  path: std
revision: 2
---
# Story: effectful-list-map

## Outcome

`List.map` and its siblings take a fn that has effects when the caller has those effects, so a
store read mapped over a list checks instead of E017.

## Context

- Row E5 found by the example program ember (`MGET`), `examples/friction.md` in the sibling
  examples repository; today the workaround is a `var` and a `for` loop.
- Related: `docs/slice.md` 20.13 defers passing a closure that uses a captured capability as a fn
  value (E017); this story is the effect-polymorphic signature of the std higher-order fns.

## Acceptance

- `ids.map(|id| db.lookup(id))` inside a fn that declares the store effect checks and runs on both
  targets; the same call in a pure fn is still E017.

## Out of Scope

- Effect inference for user-defined higher-order fns beyond what the contract names.
