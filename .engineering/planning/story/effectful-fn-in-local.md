---
format: aep.planning-md/3
id: story:effectful-fn-in-local
kind: story
status: implemented
title: A local holding an effectful fn passes to List.map
scope:
- confidence: cited
  path: crates/ignite-check/src/lib.rs
- confidence: cited
  path: crates/ignite-types/src/typer.rs
- confidence: cited
  path: docs/slice.md
- confidence: cited
  path: std/text.ignite
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-06T23:50:47Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-11-06T23:50:47Z", actor: "human:timo", revision: 5}
- {from: "active", to: "implemented", at: "2026-11-07T01:52:39Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1}}}
---
# Story: effectful-fn-in-local

## Outcome

A local variable holding an effectful fn can be passed to `List.map` like a closure or a named fn.

## Context

- Left by `story:effectful-list-map` (PR 72, 2026-11-06): `let g = |id| store.get(id); ids.map(g)` is E040; closures and named fns passed directly work. The checker does not charge the effects of a fn value held in a local.
- Also seen there: an unannotated closure parameter that calls a `Store` method gives E042 even when `map` supplies its type (`crates/ignite-types/src/typer.rs`, `effect_site`).

## Acceptance

## Acceptance

Scoped 2026-11-07 on main df7be28c: `map_fn_effects` (`crates/ignite-types/src/typer.rs:3484`) skips locals (E040); an unannotated `|s| s.get(1)` passed to `map` gets E042 because ignite-check records no effect site (`crates/ignite-check/src/lib.rs:2672`).

- Inline test in `std/text.ignite` beside "List.map takes a fn with the effects of its caller": inside `requires [store.read]`, `let g = |id: Int| store.get(id)` then `ids.map(g)` returns the stored values; `ignite test` passes it on LLVM and Rust.
- Fixture with `.expected` under `crates/ignite-types/tests/fixtures/`: the same code under `requires [store.write]` gives exactly one E016, at the closure literal (today's rule for a let-bound closure).
- `stores.map(|s| s.get(1))` without annotation gives the 20.4.3 E017 on its parameter and no E042 (added to `e017_effectful_list_map`).
- `docs/slice.md` 20.4.3 "Not covered" updated; `task gate` green.

## Out of Scope

- `Option.map`, `Result.map` and the other std higher-order fns.
