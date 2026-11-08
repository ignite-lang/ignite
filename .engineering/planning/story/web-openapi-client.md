---
format: aep.planning-md/3
id: story:web-openapi-client
kind: story
status: draft
title: OpenAPI at check time, the in-process test client, the todo example on x.web
relations:
- decomposes: epic:x-web
- depends_on: story:web-collection
- depends_on: story:web-policies
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: crates/ignite-eval/src/explain.rs
- confidence: inferred
  path: crates/ignite/src/main.rs
- confidence: inferred
  path: crates/ignite/tests/golden
- confidence: inferred
  path: crates/ignite/tests/image.rs
- confidence: inferred
  path: docs/slice.md
- confidence: inferred
  path: examples/todo/health.ignite
- confidence: inferred
  path: examples/todo/todo.ignite
- confidence: inferred
  path: tools/ignite-perf/src
- confidence: inferred
  path: x/web/client.ignite
- confidence: inferred
  path: x/web/openapi.ignite
revision: 3
---
# Story: web-openapi-client

## Outcome

`ignite explain FILE Sys.server.openapi --raw` prints the OpenAPI 3.1 document of a `Web` component's route table at check time; `client(Sys.server)` in a test serves the component over `Pipe.pair()` through the real `Http.serve` and sends typed requests; and `examples/todo` is rewritten in the declared form on `x.web` with its tests on `client` (`docs/web.md` §6.8, §8 rows 14 and 15).

## Context

- web §6.8: `--raw` prints a `Text` property verbatim (a generic CLI addition); `let made: Todo = api.post("/todos", New<Todo> { title = "milk" })?`; `api.get("/").body` for pages.
- web §8 row 15 `todo-on-web`: `examples/todo` in the declared form (web appendix A.9), durable through `Durable<Todo>` (`story:durable-store`), its hand-written probe `examples/todo/health.ignite` replaced by the derived `server.health` (`story:web-collection`).
- `examples/todo` is used by `tools/ignite-perf` (`programs`, `apply-cycle`), the toolchain image check (`todod` answers) and `crates/ignite/tests/` todo suites; the executable names they use stay or change with them in this story.

## Acceptance

- `ignite explain examples/todo/todo.ignite TodoService.server.openapi --raw` prints a document with `openapi: 3.1.0`, one path item per route with its methods, request and response schemas from the derived codecs, and the problem responses; golden file in `crates/ignite/tests/golden/`; `--raw` on a non-`Text` property is exit 2 with a message.
- An inline test using `client(TodoService.server)` creates, lists, patches and deletes a todo with typed values on both targets; a decode failure of the answer is an `Err`, not a crash.
- `examples/todo/todo.ignite` uses `Web`, `collection` and `Durable<Todo>`; its persistence code and hand-written codecs are gone; `ignite build` still emits `todod` and the probe executable the perf harness and image check name (or both are updated in this change); `crates/ignite/tests/` todo tests, `task compile-todo` and `task compile-todo-llvm` pass.
- `tools/ignite-perf self-check` and `smoke` pass on the new example.
- `docs/slice.md` section; `AGENTS.md` layout rows for `examples/todo/`; `task gate` green.

## Scope

- `x/web/openapi.ignite`, `x/web/client.ignite` (new).
- `crates/ignite/src/main.rs` (`--raw`), `crates/ignite-eval/src/explain.rs`.
- `examples/todo/todo.ignite`, `examples/todo/health.ignite` (removed or replaced), `crates/ignite/tests/` (todo suites), `crates/ignite/tests/golden/`, `tools/ignite-perf/src` (if names change), `crates/ignite/tests/image.rs` (if names change).
- `docs/slice.md`, `AGENTS.md`. Inferred from web §8 rows 14 and 15.
- Ordering: after `story:web-policies` (the todo example uses the `Web` kind with its policy defaults; both edit `docs/slice.md`), before `story:markup-views` (shared `crates/ignite-eval/src`).
