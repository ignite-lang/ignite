---
format: aep.planning-md/3
id: story:route-annotations
kind: story
status: draft
title: Function annotations, pattern literals and handlers that answer
relations:
- decomposes: epic:x-web
- depends_on: story:web-core
- depends_on: story:eval-closures-and-members
scope:
- confidence: inferred
  path: crates/ignite-check/src
- confidence: inferred
  path: crates/ignite-eval/src
- confidence: inferred
  path: crates/ignite-syntax/src
- confidence: inferred
  path: crates/ignite-types/src
- confidence: inferred
  path: crates/ignite/src/apply/plan.rs
- confidence: inferred
  path: crates/ignite/tests/golden
- confidence: inferred
  path: docs/slice.md
- confidence: inferred
  path: docs/syntax.md
- confidence: inferred
  path: x/web.ignite
revision: 3
---
# Story: route-annotations

## Outcome

A function carries annotations (`@get("/todos/{id}") fn show(id: Int) = …`), a text literal where a `Pattern` is expected keeps `{id}` as a parameter instead of an interpolation, `module()` lists annotated functions so `x.web.serve(state, module())` serves them, and a route function's result is inferred with `?` on `None` answering 404 and on `Err(fail)` answering that problem (X8 and X4 of `docs/web.md` §5).

## Context

- web §5 rows X8 (annotations as values the evaluator reads, pattern literals; today `"/todos/{id}"` is E001, appendix B) and X4 (inferred results for route functions only, Q3; `?` on `Option` through the error type's declared `none()`, a named associated fn, no implicit conversion; syntax P3).
- web §6.4: parameters bind by name (pattern parameter, body, query fields; headers only when listed), status from the method (POST 201, `Unit` 204, else 200), `status = 202` overrides.
- Generic in the compiler (layering rule 5): an annotation is any expression; nothing names HTTP.

## Acceptance

- The handlers of web §6.4 written with an explicit `todos` parameter (no `given`) and an explicit `NewTodo` patch type check, build on both targets and pass inline tests over `Pipe.pair()`: `GET /todos/7` for a missing id is 404 problem+json; `POST` is 201; `DELETE` returning `Unit` is 204; `status = 202` gives 202; a handler returning `Err(conflict("x"))` is 409 with `detail` `x`.
- `@get("/todos/{id}") fn show(nope: Int)` is a check error naming `nope` and the pattern (fixture with `.expected`); a query parameter never binds a header that the annotation does not list (inline test).
- `let p: Pattern = "/todos/{id}"` checks (appendix B's E001 case now passes), and `"/x/{id}"` where `Text` is expected is still an interpolation (fixtures).
- A non-route fn with an `=` body and no result type is still the existing error (Q3 default; fixture).
- The evaluator reads annotations: `ignite plan` of a program with `serve(todos, module())` lists the annotated routes in the model (golden).
- Evaluator, Rust and LLVM agree on the answers of one annotated program (a test in `crates/ignite/tests/`).
- `docs/slice.md` section, `docs/syntax.md` P3 updated; `CODEGEN_REVISION` bumped (`module()` records with annotations and `?` on `Option` change the generated code); `task gate` green.

## Scope

- `crates/ignite-syntax/src` (annotations, pattern literals), `crates/ignite-check/src`, `crates/ignite-types/src` (inferred route results, `?` on `Option`), `crates/ignite-eval/src` (annotations as values, `module()`).
- `x/web.ignite` (`serve`, `get`, `post`, `patch`, `delete` as annotation values).
- `crates/ignite/tests/`, `crates/ignite-check/tests/` fixtures, `crates/ignite/tests/golden/`.
- `docs/slice.md`, `docs/syntax.md`, `crates/ignite/src/apply/plan.rs` (`CODEGEN_REVISION`).
- Ordering: after `story:eval-closures-and-members` (the plan encodes the annotated functions as fn values, and both land on `crates/ignite-eval/src`); before `story:given-and-defaults` and `story:markup-views`, which change the same frontend crates and `docs/syntax.md` (web §8: a wave takes at most two frontend stories). Inferred from web §8 rows 8 and 12.
