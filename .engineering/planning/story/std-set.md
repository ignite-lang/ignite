---
format: aep.planning-md/3
id: story:std-set
kind: story
status: active
title: A Set of key values
relations:
- informed_by: story:list-sort
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-12T20:26:59Z", actor: "human:timo", revision: 2, executor: "agent:claude-coordinator", correlation: "accepted-2026-11-12"}
- {from: "proposed", to: "active", at: "2026-11-12T20:26:59Z", actor: "human:timo", revision: 3, executor: "agent:claude-coordinator", correlation: "accepted-2026-11-12"}
---
# Story: a Set of key values

## Outcome

A program keeps a set of values of any type a `Map` key may have: `Set<T>` with `empty()`, `insert`, `remove`, `contains`, `len()`, iteration in key order, and `union`, `intersect` and `minus`, on the evaluator and both targets.

## Why

Requested on 2026-11-12 by the author of a command-line tool written in Ignite: at v0.16.2 there is no `Set` type in the language or in `std`; the nearest form is a `Map<T, Bool>`, with the three set operations written as loops.

## Acceptance

- `Set<T>` accepts the key types of `Map` (`docs/slice.md` O11) and refuses any other with the diagnostic `Map` gives.
- `insert` of a present value and `remove` of an absent one leave the set unchanged and say so in their result; `for x in s` visits the values in key order.
- `union`, `intersect` and `minus` are tested against a model built from `Map` on generated inputs.
- The design step decides whether `Set` is a builtin type or a `std` type over `Map`, with the cost of each stated in allocations per `insert` on both targets; the decision is written in `docs/slice.md`.
- The evaluator, the Rust target and the LLVM target give identical output for one differential program; `task gate` green.

## Scope

- `std/` (a new module) or `crates/ignite-types`, `crates/ignite-rustgen`, `crates/ignite-llvm` when builtin; `docs/slice.md`. Inferred.

## Not in this story

- Tuples and records as keys (`story:map-composite-keys`).
