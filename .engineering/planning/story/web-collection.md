---
format: aep.planning-md/3
id: story:web-collection
kind: story
status: draft
title: The Web kind, collection routes over New<T> and Patch<T>, derived probe and contracts
relations:
- decomposes: epic:x-web
- depends_on: story:web-core
- depends_on: story:eval-closures-and-members
- depends_on: story:durable-store
- depends_on: story:data-derived-codec
- depends_on: story:route-annotations
scope:
- confidence: inferred
  path: crates/ignite-eval/src
- confidence: inferred
  path: crates/ignite-types/src
- confidence: inferred
  path: crates/ignite/src/apply/plan.rs
- confidence: inferred
  path: crates/ignite/tests/golden
- confidence: inferred
  path: docs/slice.md
- confidence: inferred
  path: x/web.ignite
- confidence: inferred
  path: x/web/probe.ignite
revision: 3
---
# Story: web-collection

## Outcome

A program declares `component server: Web { listener, routes = { "/todos" = collection(todos) }, availability = 99%, executable = "todod" }` and gets the CRUD routes of the collection with JSON bodies from mapped record types `New<T>` and `Patch<T>`, and the derived members `server.health` and `server.reachable` in the system model (X2 and the `Web` kind of `docs/web.md` §6.1, §6.2, §6.5).

## Context

- web §5 row X2: `New<T>` is `T` without the field the store assigns and with defaulted fields optional; `Patch<T>` makes every field optional; `t.with(p)` applies a patch; their codecs and schemas are derived (`story:data-derived-codec`). Q1 default: the assigned field is `id: Int`, else `New<T>` is `T`.
- web §6.1 the `Web` kind: `Web.main` runs `Http.serve` with an `HttpApp` from the routes; `machine` and `validate` are `HttpServer`'s (slice §16.16); `members` (X9, `story:eval-closures-and-members`) derives the probe and contract. The route table is a property holding closures (X3, same story).
- web §6.5: `availability` derives `server.health` (an HTTP probe of `x.web`, the shape of `examples/todo/health.ignite`), the route `/health` and `server.reachable: Availability` rolling 5 minutes; `latency = { "/todos" = 200 ms }` derives one probe and contract per route.
- `collection(store)` takes a `Store<T>` or a `Durable<T>` (`story:durable-store`).

## Acceptance

- The program of web §1 without the `"/" = page(board)` route checks silently and builds on both targets; inline tests over `Pipe.pair()`: `POST /todos {"title":"milk"}` 201 with `id` and `done: false`; `GET /todos/{id}` 200; `PATCH {"done":true}` changes only `done`; `DELETE` 204, then `GET` 404; `POST {"title":3}` 400 naming `$.title`; `POST` with an `id` member is 400 (`New<Todo>` has no `id`).
- `check = f` on `collection` runs on every create and patch; an `Err` is its answer (inline test).
- Types: `New<Todo>` and `Patch<New<Todo>>` as written types check; `t.with(p)` updates only the present fields; a `Patch` of a non-record is a check error (fixtures); evaluator, Rust and LLVM give equal values (test in `crates/ignite/tests/`).
- The same program with `Durable<Todo>` keeps its data across a restart (end-to-end test).
- `ignite plan` lists `TodoService.server.health` and `TodoService.server.reachable` with `derived by TodoService.server` (golden); `latency = { "/todos" = 200 ms }` adds one probe and contract for that route; `ignite run --clock manual` schedules the probe and reports the standing (end-to-end test).
- `Web.routes` also serves the module's annotated functions (`routes = module()` merged with the route map, `story:route-annotations`): an inline test serves `@get("/hello") fn hello() = "hi"` beside `collection(todos)` on one `Web` component.
- `shape = Shape { keys = Camel }` on the component renames the wire members of every route's codec: a field `due_at` is `dueAt` in `POST` bodies and answers (inline test).
- `docs/slice.md` section; `CODEGEN_REVISION` bumped (the derived codecs of `New<T>` and `Patch<T>` are new generated code); `task gate` green.

## Scope

- `crates/ignite-types/src` (mapped record types, `with`), `crates/ignite-eval/src` (if the mapped types reach the evaluator).
- `x/web.ignite` (`Web`, `collection`, `availability`, `latency`), `x/web/probe.ignite` (new: the HTTP probe kind).
- `crates/ignite/tests/`, `crates/ignite/tests/golden/`, `crates/ignite-check/tests/` fixtures.
- `docs/slice.md`, `crates/ignite/src/apply/plan.rs` (`CODEGEN_REVISION`).
- Ordering: after `story:route-annotations` (the join of annotated functions into `Web.routes`) and `story:durable-store`; before `story:web-policies`, which adds the policy fields to this story's `Web` kind. Inferred from web §8 row 5.
