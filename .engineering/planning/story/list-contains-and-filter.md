---
format: aep.planning-md/3
id: story:list-contains-and-filter
kind: story
status: active
title: A List has contains, any, all, find and filter
relations:
- informed_by: story:list-sort
scope:
- confidence: cited
  path: crates/ignite-check/src/lib.rs
- confidence: cited
  path: crates/ignite-eval/src/eval.rs
- confidence: cited
  path: crates/ignite-llvm/src/lower.rs
- confidence: cited
  path: crates/ignite-rustgen/src/lib.rs
- confidence: cited
  path: crates/ignite-types/src/builtins.rs
- confidence: cited
  path: crates/ignite-types/src/lib.rs
- confidence: cited
  path: crates/ignite-types/src/ownership.rs
- confidence: cited
  path: crates/ignite-types/src/typer.rs
- confidence: cited
  path: crates/ignite-types/tests/builtin_tables.rs
- confidence: inferred
  path: crates/ignite/tests/fixtures/list_contains_filter.ignite
- confidence: inferred
  path: crates/ignite/tests/list_contains_filter.rs
- confidence: cited
  path: docs/friction.md
- confidence: cited
  path: docs/slice.md
- confidence: cited
  path: site/book/src/reference/types.md
- confidence: inferred
  path: std/control.ignite
revision: 19
transitions:
- {from: "draft", to: "proposed", at: "2026-11-12T20:26:59Z", actor: "human:timo", revision: 2, executor: "agent:claude-coordinator", correlation: "accepted-2026-11-12"}
- {from: "proposed", to: "active", at: "2026-11-12T20:26:59Z", actor: "human:timo", revision: 3, executor: "agent:claude-coordinator", correlation: "accepted-2026-11-12"}
---
# Story: a List has contains, any, all, find and filter

## Outcome

A `List<T>` answers `contains(x)`, `any(f)`, `all(f)`, `find(f)` and `filter(f)` as builtin methods, on the evaluator and both targets, with the same result everywhere.

## Why

Requested on 2026-11-12 by the author of a command-line tool written in Ignite: a hand-written `has` helper is its third most frequent workaround, and each filter is a `for` loop around a `push`. The builtin methods of `List` (`crates/ignite-types/src/builtins.rs` `list_method`) are `len`, `is_empty`, `at`, `slice`, `push`, `set`, `append`, `clear` and `map`. `docs/friction.md` holds both as open rows (22 `filter`, 57 `contains`).

## Acceptance

- `xs.contains(x)` for every `T` that `==` accepts; another `T` is one diagnostic that names `any`.
- `xs.any(f)` and `xs.all(f)` with `f: fn(T) -> Bool` stop at the first deciding element (a test counts the calls of `f`); on an empty list `any` is `false` and `all` is `true`.
- `xs.find(f)` gives the first element `f` accepts as `Option<T>`.
- `xs.filter(f)` gives the accepted elements in order; the source list is unchanged.
- A closure that performs an effect is accepted where `map` accepts one, and refused where `map` refuses one.
- Allocations on the Rust target: `contains`, `any`, `all` 0; `find` at most the copy of the found element; `filter` one list and one copy per kept element. Counted at two sizes; the LLVM target's counts are written down.
- The rows 22 and 57 of `docs/friction.md` move to solved; `docs/slice.md` and `site/book` list the methods; `task gate` green.

## Scope

Derived 2026-11-23 by `story-scoper` against `wave/16` (`e48f64a5`). Every line is **cited** (read from the story or the tree) or **inferred**. None of `contains`, `any`, `all`, `find`, `filter` exists on `List` at that head; `contains` exists only for `Map` (`MapContains`), and the name is already in `BUILTIN_METHODS`.

- **Primary surface:** the list builtin path `story:list-sort` set up, beside `sort`/`sort_by` in each consumer — cited
- `crates/ignite-types/src/builtins.rs:308` `list_method` — cited
- `crates/ignite-types/src/typer.rs` `method` :3592 (dispatch :3609-3618, E045 arm :3640), new siblings of `list_map` :3775 and `list_sort_by` :3835; reuses `equatable` :1887, `map_fn_effects` :3932, `map_local_site` :3905, `map_effects_fit` :3966 — cited
- `crates/ignite-types/src/lib.rs` `Builtin` :705-711, `is_builtin_method` :313-316 — cited
- `crates/ignite-types/src/ownership.rs:138-140` (builtins that do not mutate) — cited
- `crates/ignite-check/src/lib.rs` `BUILTIN_METHODS` :209, `method_roots` :2332, `method` :2906, `builtin_applies` :4525-4555 — cited
- `crates/ignite-rustgen/src/lib.rs` `method` :5694 (builtin arms :5951-5998, the `ListMap` block :6016, the name table :6046-6083), `reads_element_in_place` :5173 — cited
- `crates/ignite-llvm/src/lower.rs` `builtin` :4742, siblings of `list_map` :5722 and `list_sort` :5807 — cited
- `crates/ignite-eval/src/eval.rs` `builtin_method` :1282, `builtin_value` :1384 — cited
- `crates/ignite-types/tests/builtin_tables.rs:149` — cited
- Tests `crates/ignite/tests/list_contains_filter.rs` with a fixture, built like `list_sort.rs` (differential, calls of `f`, effects, allocations) — inferred
- `std/control.ignite:793` hand-written `contains(List<Duration>, Duration)` may become the builtin — inferred
- Documents: `docs/slice.md` (a new section after 43, the table near :689, E017 :1247, E045 :1276), `docs/friction.md` rows 22 and 57, `site/book/src/reference/types.md` — cited
- **Confidence:** high.
- **Collides with:** a unit editing the Rust generator's `method` or `reads_element_in_place`, the method dispatch of `typer.rs`, the `Builtin` enum, `builtin_applies`/`BUILTIN_METHODS`, `Lower::builtin`, `builtin_method`.
- **Not established:** whether the methods also apply to `Bytes`; the diagnostic code of a wrong element type for `contains` (E045 by the `sort` precedent).

## Not in this story

- `sort` (`story:list-sort`), a `Set` (`story:std-set`), `fold`, `zip`, `enumerate`.
