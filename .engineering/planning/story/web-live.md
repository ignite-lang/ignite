---
format: aep.planning-md/3
id: story:web-live
kind: story
status: draft
title: 'Effect sets as values: ETags and live pages without subscriptions'
relations:
- decomposes: epic:x-web
- depends_on: story:markup-views
- depends_on: story:http-app-connection
- depends_on: story:channels-and-timers
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
  path: crates/ignite-types/src
- confidence: inferred
  path: crates/ignite/src/apply/plan.rs
- confidence: inferred
  path: docs/slice.md
- confidence: inferred
  path: std/durable.ignite
- confidence: inferred
  path: x/web.ignite
- confidence: inferred
  path: x/web/live.ignite
revision: 5
---
# Story: web-live

## Outcome

The evaluator reads each route function's inferred read set, mapped to the resources its parameters are bound to (X11); from it `x.web` answers a GET whose read set holds only stores with an `ETag` of the stores' versions and 304 on a match, and a `page(view)` updates in every open browser after any write to a store the view reads, with no subscription written (`docs/web.md` §6.3).

## Context

- web §5 row X11 (`ignite-check`'s effect table exposed as `Symbols.effects`, read by `crates/ignite-eval`); web §6.3: `Store.version()` is a substrate addition each write raises; a read set with a `Directory`, a `Launcher` or a `Service` gets no ETag.
- Live: the page carries a fixed script of `x.web.live` (a static asset under 2 KB, no build step, the one hand-written non-Ignite file `x` holds, web §3) that opens one event stream per page; after a write completes to a store in the view's read set, from any route or task, the server renders the view again and sends it; writes in one request coalesce to one render; `live = false` turns it off. Q5 default: the whole view rendered, changed elements replaced in the browser.
- The event stream holds a connection open (`story:http-app-connection`) and waits for a write (`story:channels-and-timers`).
- The `ETag` is slot 12 of web §6.11.

## Acceptance

- Inline tests over `Pipe.pair()` on both targets: `GET /todos` carries an `ETag`; the same request with `If-None-Match` of it is 304 with no body; after a `POST` the ETag differs; a route reading a `Directory` has no `ETag`; a route reading two stores changes its ETag after a write to either and not after a write to a third store it does not read (the read set, not all stores, is the key); `Store.version()` rises by one per write.
- `Durable<T>` (Q6 default): the ETag of a route reading a `Durable` store is derived from the version of the newest file; two processes over one directory answer the same ETag after a write by either and a reload (end-to-end test, one target). The second half of Q6, no ETag during a rollout's drain, is not built here: a process cannot tell that a second instance runs; recorded as a known limit in `docs/slice.md` beside slice §22.2's.
- Live, an end-to-end test over TCP on both targets: two clients open `/` and its event stream; a `POST /todos` from a third connection makes each stream deliver one event within 500 ms whose data is the re-rendered `board` containing the new title; a request with two writes delivers one event; `live = false` serves no script and no stream; closing a client ends its stream on the server (the next write does not block).
- The script is served with a fixed `Content-Type`, is under 2048 bytes (test), and replaces the elements that changed; its DOM behaviour is not run in the gate (no browser in `cargo test`): recorded in `docs/slice.md`.
- `docs/slice.md` section; `CODEGEN_REVISION` bumped (`Store.version`); `task gate` green.

## Scope

- `crates/ignite-check/src` (`Symbols.effects`), `crates/ignite-eval/src` (read sets as values).
- `Store.version`: `crates/ignite-io/src/store.rs`, `crates/ignite-rt/src`, `crates/ignite-types/src`, `crates/ignite-llvm/src`, `crates/ignite-rustgen/src`, `crates/ignite/src/apply/plan.rs`. Collides with `story:store-api-gaps` on `store.rs`.
- `x/web/live.ignite` (new, with the script as a constant), `x/web.ignite` (ETag, `page` live).
- `std/durable.ignite` (the version of a `Durable` store).
- `crates/ignite/tests/`, `docs/slice.md`. Inferred from web §8 row 13.
- Ordering: after `story:markup-views`; before `story:web-markdown`, which shares `x/web.ignite`, `crates/ignite-eval/src`, `crates/ignite-check/src`, `crates/ignite-types/src` and `plan.rs`.
