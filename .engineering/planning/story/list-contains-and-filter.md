---
format: aep.planning-md/3
id: story:list-contains-and-filter
kind: story
status: draft
title: A List has contains, any, all, find and filter
relations:
- informed_by: story:list-sort
revision: 1
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

- `crates/ignite-types/src/builtins.rs`, `typer.rs`, `crates/ignite-check/src/lib.rs` (the name list), `crates/ignite-rustgen/src/lib.rs`, `crates/ignite-llvm/`, the evaluator, `docs/slice.md`, `docs/friction.md`. Inferred.

## Not in this story

- `sort` (`story:list-sort`), a `Set` (`story:std-set`), `fold`, `zip`, `enumerate`.
