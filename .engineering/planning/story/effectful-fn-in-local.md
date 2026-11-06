---
format: aep.planning-md/3
id: story:effectful-fn-in-local
kind: story
status: draft
title: A local holding an effectful fn passes to List.map
revision: 1
---
# Story: effectful-fn-in-local

## Outcome

A local variable holding an effectful fn can be passed to `List.map` like a closure or a named fn.

## Context

- Left by `story:effectful-list-map` (PR 72, 2026-11-06): `let g = |id| store.get(id); ids.map(g)` is E040; closures and named fns passed directly work. The checker does not charge the effects of a fn value held in a local.
- Also seen there: an unannotated closure parameter that calls a `Store` method gives E042 even when `map` supplies its type (`crates/ignite-types/src/typer.rs`, `effect_site`).

## Acceptance

- The call above checks inside a fn whose `requires` covers the effect and runs on both targets (inline test); E017 outside; `|s| s.get(1)` without annotation types when `map` gives the element type; `task gate` green.

## Out of Scope

- `Option.map`, `Result.map` and the other std higher-order fns.
