---
format: aep.planning-md/3
id: story:given-and-defaults
kind: story
status: draft
title: given, parameter defaults, Store.insert and update from another record
relations:
- decomposes: epic:x-web
- depends_on: story:data-derived-codec
- depends_on: story:route-annotations
scope:
- confidence: inferred
  path: crates/ignite-check/src
- confidence: inferred
  path: crates/ignite-eval/src
- confidence: inferred
  path: crates/ignite-io/src/store.rs
- confidence: inferred
  path: crates/ignite-llvm/src
- confidence: inferred
  path: crates/ignite-rt/src
- confidence: inferred
  path: crates/ignite-rustgen/src
- confidence: inferred
  path: crates/ignite-syntax/src
- confidence: inferred
  path: crates/ignite-types/src
- confidence: inferred
  path: crates/ignite/src/apply/plan.rs
- confidence: inferred
  path: docs/slice.md
- confidence: inferred
  path: docs/syntax.md
revision: 3
---
# Story: given-and-defaults

## Outcome

Three small language extensions of `docs/web.md` §5: `given todos: Store<Todo>` makes `todos` a leading parameter of every function of the module that uses it (X5); a parameter may have a default, including one whose type mentions a type parameter typed at the call site, `codec: Codec<P> = codec()` (X6, syntax P10); `todos.insert(|id| Todo { id, ..new })` assigns the id and a record update takes its base from another record type with the same field names (X7).

## Context

- web §5 rows X5 (desugared before effect inference, so effects stay on a parameter; Q7: a `given` is local to its module), X6 (syntax P10's defaults; slice §14.2 took named arguments only), X7 (a `Store` builtin; record update from another record).
- Savings: −33, −31, −18 tokens on the reference service (web §5).
- X6's motivating default calls `std.data.codec()` (`story:data-derived-codec`).

## Acceptance

- `given`: a module with `given todos: Store<Todo>` and two fns that use `todos` and one that does not; callers in another module pass `todos` explicitly; the unused fn takes no parameter (inline tests on both targets); an ambient write through a `given` in a `GET` route is still E017 (fixture); a `given` used in another module without passing it is a check error (fixture).
- Defaults: `fn f(a: Int, b: Int = 2)` called as `f(1)` and `f(1, b = 3)`; `fn decode_as<P>(v: Value, codec: Codec<P> = codec()) -> Result<P, DecodeError>` called with `P` fixed by the expected type (inline tests on both targets); a default that does not type is a check error at the declaration (fixture).
- `Store.insert(f)` returns the new value with the next id and stores it; `Todo { id, ..new }` with `new: NewTodo` copies the shared fields; a field of `NewTodo` missing from `Todo` is a check error (fixture). Inline tests on both targets and the evaluator.
- `docs/slice.md` sections, `docs/syntax.md` P10 marked taken; `CODEGEN_REVISION` bumped (`Store.insert`); `task gate` green.

## Scope

- `crates/ignite-syntax/src`, `crates/ignite-check/src`, `crates/ignite-types/src`, `crates/ignite-eval/src` (the builtin in the evaluator).
- `Store.insert`: `crates/ignite-io/src/store.rs`, `crates/ignite-rt/src`, `crates/ignite-llvm/src`, `crates/ignite-rustgen/src`, `crates/ignite/src/apply/plan.rs` (`CODEGEN_REVISION`). Collides with `story:store-api-gaps` on `store.rs`.
- `docs/slice.md`, `docs/syntax.md`. Inferred from web §8 rows 9, 10, 11.
- Ordering: after `story:route-annotations` (and so after `story:eval-closures-and-members`), which share the frontend crates, `crates/ignite-eval/src` and `docs/syntax.md`; before `story:durable-store`, whose interface uses `Store.insert`.
