---
format: aep.planning-md/3
id: story:drain-keeps-listeners
kind: story
status: proposed
title: A listener can stay open through the SIGTERM drain
revision: 2
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T01:25:58Z", actor: "human:timo", revision: 2}
---
# Story: drain-keeps-listeners

## Outcome

A listener can be declared to stay open through the SIGTERM drain, so an admin port answers `/readyz` with 503 and internal listeners keep working while public ingress drains.

## Context

- Case study gap G6: `crates/ignite-io/src/drain.rs` `begin()` and `release_all()` release every listener; the service re-binds its ports after "draining".

## Acceptance

- A `TcpListener` resource or bind call can say it is kept through the drain (e.g. `drain = Keep`); kept listeners accept until the process exits; the others are released as today.
- G6's reproducer: after SIGTERM the kept listener still accepts and the ingress listener is released; on both targets.
- The drain window still ends the process when every non-kept connection is done (story drain-exit unchanged).
- `docs/slice.md` drain section updated; `task gate` green.

## Scope

- `crates/ignite-io/src/drain.rs`, `net.rs`, `crates/ignite-rt`, `std/net.ignite`, both backends' resource binding, `docs/slice.md`. Inferred.
