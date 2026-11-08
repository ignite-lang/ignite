---
format: aep.planning-md/3
id: story:web-policies
kind: story
status: draft
title: x.web policies in one derived order, and task deadlines
relations:
- decomposes: epic:x-web
- depends_on: story:http-app-connection
- depends_on: story:web-collection
scope:
- confidence: inferred
  path: crates/ignite-check/src
- confidence: inferred
  path: crates/ignite-io/src
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
  path: crates/ignite/tests
- confidence: inferred
  path: docs/slice.md
- confidence: inferred
  path: std/net.ignite
- confidence: inferred
  path: std/protocol/http.ignite
- confidence: inferred
  path: x/web.ignite
- confidence: inferred
  path: x/web/gzip.ignite
- confidence: inferred
  path: x/web/policy.ignite
revision: 5
---
# Story: web-policies

## Outcome

The `Web` kind applies request id, access log, limits, CORS, a per-client rate limit, the program's `before` check, a request timeout, problem+json errors and compression in the one fixed order of `docs/web.md` §6.11, each a property of the `Web` component with a default, overridable per route by annotation and per `scope`, with nothing for the program to order. A task has a deadline after which every blocking call fails with `TimedOut`, which the timeout slot uses.

## Context

- web §6.11 (slots 1 to 5, 9, 10, 13, 14; slots 6 to 8 are identity, `story:web-identity` and `story:web-sessions`; slot 12, the cache, is `story:web-live`). Measured: −191 tokens against Hono (web §2.6, appendix A.11).
- The properties are fields of the `Web` kind (web §6.1), which `story:web-collection` creates; per-route overrides are annotation arguments (`story:route-annotations`).
- Substrate (web §6.11 last paragraph, web §8 `task-deadlines`): a deadline per task in `crates/ignite-io`; the handler's thread ends at its next IO call; a loop with no IO is not stopped (recorded limit). This is the one substrate change of the story; it is kept here because slot 10 is its only user.
- The per-client limit keys on the peer address, which `story:http-app-connection` gives the handler; `trusted_proxy` is the only way a forwarded address is read.
- Access logging on the server is web §8 `http-access-log`: standard error is no handler's capability, so the server writes the line.
- Ordering: after `story:web-collection` (the `Web` kind) and before `story:web-openapi-client`; it shares `crates/ignite-check/src`, `crates/ignite-types/src`, the backends and `CODEGEN_REVISION` with `story:route-annotations`, `story:given-and-defaults` and `story:web-live`, which the `depends_on` chain of `epic:x-web` orders.

## Acceptance

- Inline tests over `Pipe.pair()` on both targets, one per slot, on a `Web` component:
  - `X-Request-Id` of 1 to 64 `[A-Za-z0-9-]` characters is echoed; any other value is replaced by 32 hex characters from `Entropy`.
  - `cors = ["https://app.example.org"]`: a preflight from that origin gets `Access-Control-Allow-Methods` from the route table and no handler runs; another origin gets no CORS headers.
  - `limit = per_client(2, 1 minute)`: the third request from one peer is 429 with `Retry-After`; another peer is admitted.
  - `trusted_proxy = "10.0.0.1"`: a request from peer `10.0.0.1` with `X-Forwarded-For: 192.0.2.7` is limited as `192.0.2.7`; the same header from any other peer is ignored and the peer address is the key; without `trusted_proxy` the header is never read.
  - `timeout = 1 seconds` on a handler blocked in a read: 503 within 1.5 s; the server keeps serving.
  - `@post("/upload", body = 10 MiB)` and a `scope` override each change the answer for their routes only. This story builds the route constructor `scope(pattern, routes, …)` of web §6.2 and §6.11 (web §8 row 16).
- Order, inline tests: a 429 and a 503 are problem+json whose `instance` is the request id; a request refused by `per_client` never reaches `before` (a counter in `before` stays 0); a preflight is answered without `before` running; a 2 KiB problem body to a request accepting gzip is compressed (compression runs after errors).
- Access log and compression, an end-to-end test in `crates/ignite/tests/` that builds a `Web` program on both targets, runs it over TCP and reads the process's standard error: one line per request with id, method, pattern (not the raw path), status and duration; a 2 KiB text body to `Accept-Encoding: gzip` is restored byte for byte by `gzip -dc` (no new dependency); a 500-byte body and an event stream are sent uncompressed.
- E120 fixtures with `.expected`: a component `timeout` longer than `rollout.deadline`; a route annotation's `timeout` longer than it; a forwarded address read without `trusted_proxy`.
- Task deadline: a Rust test in `crates/ignite-io` and an inline test on both targets: a read on a pipe with a deadline 100 ms ahead fails with `TimedOut` within 300 ms; without a deadline the behaviour is unchanged.
- `docs/slice.md` section; `CODEGEN_REVISION` bumped for the deadline builtin; `task gate` green.

## Scope

- `x/web.ignite`, `x/web/policy.ignite` (new), `x/web/gzip.ignite` (new: deflate per RFC 1951/1952 in Ignite).
- `std/protocol/http.ignite` (access log on the server).
- Deadline: `crates/ignite-io/src`, `crates/ignite-rt/src`, `crates/ignite-check/src` and `crates/ignite-types/src` (builtin), `crates/ignite-llvm/src`, `crates/ignite-rustgen/src`, `std/net.ignite`, `crates/ignite/src/apply/plan.rs` (`CODEGEN_REVISION`).
- `crates/ignite/tests/` (end-to-end), `crates/ignite-check/tests/` fixtures, `docs/slice.md`. All inferred from web §6.11 and §8 rows 16 and `task-deadlines`.
