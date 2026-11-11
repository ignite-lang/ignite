---
format: aep.planning-md/3
id: story:std-resp-client
kind: story
status: implemented
title: 'std.protocol.resp: RESP3 client with pub/sub and Sentinel'
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T01:25:59Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-10T01:15:24Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-11-10T19:39:18Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}}
---
# Story: std-resp-client

## Outcome

`std.protocol.resp` is a RESP2/RESP3 client over any `ByteStream`: commands, pipelining, pub/sub subscription and Sentinel master discovery, so a service stores sessions and fans out messages through a Redis-compatible server without hand-written framing.

## Context

- Case study gap G9 (related list): no Redis or Sentinel client in std.

## Acceptance

- Encoder and decoder for every RESP3 type with limits (`RespLimits`); commands as typed helpers for the strings, hashes, sets, keys, expiry and pub/sub groups; pipelining returns replies in order.
- A subscription yields messages on a task until unsubscribed or closed; reconnect is the caller's, with a documented helper.
- Sentinel: ask a list of sentinels for the master of a name and follow a `+switch-master` message.
- Tests over `Pipe.pair()` with a scripted server; one test against a real `redis-server` and `redis-sentinel` when present (ignored elsewhere, like `image::`).
- Layering rule kept; `task gate` green.

## Scope

- `std/protocol/resp.ignite` (new), `docs/layering.md`, `crates/ignite-check/tests/layering.rs`, `docs/slice.md`. Inferred.
