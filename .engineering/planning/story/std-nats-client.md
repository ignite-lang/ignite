---
format: aep.planning-md/3
id: story:std-nats-client
kind: story
status: active
title: 'std.protocol.nats: NATS core client'
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T01:26:00Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-10T01:15:24Z", actor: "human:timo", revision: 4}
---
# Story: std-nats-client

## Outcome

`std.protocol.nats` is a NATS core client over any `ByteStream`: connect with auth options, publish, subscribe with queue groups, request/reply, ping/pong keepalive, so a service exchanges messages with a NATS server without hand-written framing.

## Context

- Case study gap G9 (related list): no NATS client in std.

## Acceptance

- INFO/CONNECT handshake (user/password, token), PUB, HPUB, SUB, UNSUB, MSG, HMSG, PING/PONG, -ERR handling, limits (`NatsLimits`); subjects with wildcards matched as the server does.
- Reconnect with a bounded outgoing buffer: messages older than a configured age are dropped and counted instead of sent late (the spec of the case study: drop and count after 1 s); a reconnect never replays stale messages.
- Request/reply with an inbox and a deadline (uses story channels-and-timers when landed, a read timeout before).
- Tests over `Pipe.pair()` with a scripted server; one test against a real `nats-server` when present (ignored elsewhere).
- Layering rule kept; `task gate` green.

## Scope

- `std/protocol/nats.ignite` (new), `docs/layering.md`, `crates/ignite-check/tests/layering.rs`, `docs/slice.md`. Inferred.
