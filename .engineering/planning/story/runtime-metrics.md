---
format: aep.planning-md/3
id: story:runtime-metrics
kind: story
status: proposed
title: A server exports its runtime census as metrics
summary: lines(census, prefix) in std.runtime; the todo example serves GET /runtime
relations:
- decomposes: epic:leak-guard
- depends_on: story:leak-census
scope:
- confidence: inferred
  path: crates/ignite/tests
- confidence: inferred
  path: docs/slice.md
- confidence: inferred
  path: examples/todo/todo.ignite
- confidence: inferred
  path: std/runtime.ignite
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T15:04:31Z", actor: "human:timo", revision: 3}
---
# Story: runtime-metrics

## Outcome

A running Ignite server exports its census as metrics: `lines(Runtime.census(), prefix)` gives one Prometheus text line per kind, and the todo example serves it at `GET /runtime`, so a server that grows in production shows which kind grows.

## Context

- Design: `docs/leak-guard.md` sections 5.4 and 6.4, Q9 (resident size reported, never compared), Q11 (handle slots as the table's high-water mark).
- Builds on `story:leak-census`'s `Runtime.census()`; text only, no import (`std.runtime` is base layer).

## Acceptance

- An inline test in `std/runtime.ignite` checks the lines of a census record on both targets (one `<prefix><kind> <value>` line per kind, `heap_bytes` omitted when `None`).
- An end-to-end test builds `todod` on each target, sends `GET /runtime`, and finds `todo_tasks`, `todo_handles` (LLVM) and `todo_resident_bytes` lines; after 100 requests the `todo_streams` line is back at its value before them.
- `examples/todo/todo.ignite` serves the route; `task gate` green.

## Scope

- `std/runtime.ignite`, `examples/todo/todo.ignite`, `crates/ignite/tests/` (the todo suite), `docs/slice.md`. Inferred.
