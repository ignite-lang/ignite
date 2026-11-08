---
format: aep.planning-md/3
id: story:durable-store
kind: story
status: draft
title: 'Durable<T>: a keyed store over a Directory in std'
relations:
- decomposes: epic:x-web
- depends_on: story:data-derived-codec
- depends_on: story:given-and-defaults
- depends_on: story:web-core
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: crates/ignite-check/tests/layering.rs
- confidence: inferred
  path: crates/ignite-types/src
- confidence: inferred
  path: crates/ignite/tests/main.rs
- confidence: inferred
  path: docs/layering.md
- confidence: inferred
  path: docs/slice.md
- confidence: inferred
  path: std/durable.ignite
revision: 5
---
# Story: durable-store

## Outcome

`std` has `Durable<T>`, a keyed store over a `Directory` with the operations of `Store<T>` that `x.web` uses, writing every change as `examples/todo` does today (one file per version, tombstones, sharded directories, load on first use) with the derived codec of `T` (X12 of `docs/web.md` §5). A program declares `resource todos: Durable<Todo> { directory = data }` and needs no persistence code.

## Context

- web §6.6; slice §22.4 is the format, `examples/todo/todo.ignite` (its persistence code, about 120 lines) the source it is generalised from. The known limit of slice §22.2 (two processes over one directory during a drain) and writes without `fsync` stay.
- One interface over `Store<T>` and `Durable<T>` so `collection` (`story:web-collection`) takes either; its `insert` is the `Store.insert` of `story:given-and-defaults` (today's `Store` has `next_id` and `put`, slice §11.3).
- Ordering: after `story:web-core` and `story:given-and-defaults`; this story and `web-core` both edit `docs/layering.md`, `crates/ignite-check/tests/layering.rs`, `AGENTS.md` and `crates/ignite/tests/main.rs`.
- Q6 default: the ETag of a `Durable` store across processes is the version of the newest file; that is `story:web-live`'s, not here.

## Acceptance

- Inline tests on both targets with `Directory.temporary()`: insert, update, remove, get, values; the files on disk are `<shard>/<id>/<version>.json` and a removal writes a tombstone; a second `Durable<T>` over the same directory loads the newest versions and not the removed ids.
- An end-to-end test in `crates/ignite/tests/`: a built program writes three values, is killed and restarted, and answers the three.
- A `T` with no derived codec (a field of type `TcpStream`) is a check error at the resource (fixture).
- A fn generic over the shared interface accepts both a `Store<T>` and a `Durable<T>` (inline test).
- `docs/layering.md` and `layering::` row for `std.durable`; `docs/slice.md` section; `AGENTS.md` layout row; `task gate` green.

## Scope

- `std/durable.ignite` (new); the shared interface in `std/durable.ignite` or beside `Store`'s declarations; `crates/ignite-types/src` (`Store` is a builtin type, `Ty::Store` in `builtins.rs`, so it satisfying the interface is a type-checker change).
- `docs/layering.md`, `crates/ignite-check/tests/layering.rs`, `crates/ignite/tests/` (restart test), `docs/slice.md`, `AGENTS.md`.
- `examples/todo/todo.ignite` is not changed here (`story:web-openapi-client` ports it). Inferred from web §8 row 4.
