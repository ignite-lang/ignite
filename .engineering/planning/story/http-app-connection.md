---
format: aep.planning-md/3
id: story:http-app-connection
kind: story
status: proposed
title: HttpApp sees its connection, holds, takes over and answers oversized bodies
relations:
- depends_on: story:channels-and-timers
revision: 2
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T03:32:56Z", actor: "human:timo", revision: 2}
---
# Story: http-app-connection

## Outcome

An `HttpApp` handler sees its connection, can hold a request until a message or a deadline, can take the connection over, and chooses the answer to a body over the limit, so a WebSocket or long-poll server runs on `Http.serve` instead of a hand-written one.

## Context

- Case study gap G28: the handler never sees the connection (no peer address, no takeover), cannot wait (a hold is E017), and std answers a too-large body with its own 413 before the application runs.
- `story:channels-and-timers` (wave 7) gives the waiting primitive; `story:std-websocket` builds its upgrade on the takeover.

## Acceptance

- The handler receives a connection view: peer address, local address, whether TLS; an allow-list example refuses a peer with 403.
- A handler may return `Hold(receiver, deadline, on_timeout)`: the server waits on a `std.sync` receiver for the response or answers `on_timeout` at the deadline; held requests do not block other connections; the drain answers held requests.
- A handler may return `Upgrade(response, takeover)`: the server writes the response and hands the stream to `takeover` on the connection's task.
- A hook `on_oversized(request_head, limit) -> Response` decides the answer to a body over `HttpLimits`; the server discards the body and keeps the connection alive when the answer allows it.
- The conformance claims still hold (`ignite conformance`); tests over `Pipe.pair()` for each case, one long-poll test over TCP with 1,000 held requests; `docs/slice.md` section; `task gate` green.

## Scope

- `std/protocol/http.ignite`, `std/net.ignite`, `crates/ignite-types` (effects of a held handler), `docs/slice.md`, `site/book/src/reference/std/http.md`. Inferred.
