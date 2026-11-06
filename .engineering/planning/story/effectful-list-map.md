---
format: aep.planning-md/3
id: story:effectful-list-map
kind: story
status: implemented
title: List.map over effectful fns
summary: Let List.map and its siblings take a fn with the caller's effects, closing ember's E5
scope:
- confidence: inferred
  path: crates/ignite-check/src
- confidence: inferred
  path: crates/ignite-types/src
- confidence: cited
  path: crates/ignite-types/src/typer.rs
- confidence: cited
  path: crates/ignite-types/tests
- confidence: cited
  path: docs/slice.md
- confidence: inferred
  path: std
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-06T15:04:13Z", actor: "human:timo", revision: 5}
- {from: "proposed", to: "active", at: "2026-11-06T15:04:13Z", actor: "human:timo", revision: 6}
- {from: "active", to: "implemented", at: "2026-11-06T17:15:49Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"test_result":1}}}
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

## Acceptance

Scoped 2026-11-06 from main 317955736d: effect inference (`crates/ignite-check/src/lib.rs:2745`) and both backends and the evaluator (`crates/ignite-llvm/src/lower.rs:4864`, `crates/ignite-rustgen/src/lib.rs:4351`, `crates/ignite-eval/src/eval.rs:1111`) already accept an effectful fn value; the typer's `list_map` (`crates/ignite-types/src/typer.rs:3394`) checks the argument against the pure `fn_ty`, which is the E017. "Siblings" here means `List.map` only; `Option.map`, `and_then`, `Result.map`, `map_err` stay pure (Out of Scope).

- An inline test in `std/` or `examples/`: a fn with `requires` naming a `Store` maps `ids.map(|id| store.get(id))` over a `List<Int>` and returns the results; `ignite test` passes it on LLVM and Rust.
- A checker fixture in `crates/ignite-types/tests/fixtures/` with `.expected`: the same call where the enclosing fn's `requires` does not cover the effect gives E017 at the closure.
- A fixture where `map`'s argument is a named effectful fn (not a closure) behaves the same way: checks inside a covering fn, E017 outside.
- `docs/slice.md` 20.13 states the rule as landed.
- `task gate` green.

## Out of Scope

- Effect inference for user-defined higher-order fns beyond what the contract names.
