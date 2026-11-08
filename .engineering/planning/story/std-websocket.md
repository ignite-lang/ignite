---
format: aep.planning-md/3
id: story:std-websocket
kind: story
status: proposed
title: 'std.protocol.websocket: RFC 6455 server and client'
relations:
- depends_on: story:bitwise-operators
revision: 2
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T01:25:59Z", actor: "human:timo", revision: 2}
---
# Story: std-websocket

## Outcome

`std.protocol.websocket` implements RFC 6455 server and client roles over any `ByteStream`, with the HTTP upgrade from `std.protocol.http`, so a service accepts WebSocket connections, sends and receives text, binary, ping, pong and close frames without hand-written framing.

## Context

- Case study gap G9 (related list): no WebSocket support in std; G7 made its SHA-1, base64 and masking slow.
- Depends on story bitwise-operators (SHA-1, masking).

## Acceptance

- The upgrade handshake (Sec-WebSocket-Accept, version 13, subprotocol choice) through `HttpApp`; frames: fragmentation, control frames interleaved, masking rules per role, close codes, size limits (`WebSocketLimits`).
- `std.rfc.rfc6455` catalogue imported by `ignite rfc import`; the protocol declares `conforms [RFC6455]` for the server role and `ignite conformance` reports the claim; gaps are listed, not hidden.
- Tests over `Pipe.pair()` plus one over TCP; an echo server built on both targets passes the Autobahn-style cases written as inline tests (fragmentation, ping/pong, close handshake, invalid UTF-8).
- Layering rule kept (`docs/layering.md`); `task gate` green.

## Scope

- `std/protocol/websocket.ignite` (new), `std/rfc/rfc6455.ignite` (new), `docs/rfc/`, `std/protocol/http.ignite` (upgrade hook), `docs/layering.md`, `crates/ignite-check/tests/layering.rs`, `docs/slice.md`. Inferred.
