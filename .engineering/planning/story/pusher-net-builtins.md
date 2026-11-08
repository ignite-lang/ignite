---
format: aep.planning-md/3
id: story:pusher-net-builtins
kind: story
status: implemented
title: Peer address, distinguishable read timeout, connect timeout
scope:
- confidence: inferred
  path: crates/ignite-check/src/lib.rs
- confidence: inferred
  path: crates/ignite-io/src/net.rs
- confidence: inferred
  path: crates/ignite-llvm/src
- confidence: inferred
  path: crates/ignite-rt/src
- confidence: inferred
  path: crates/ignite-rustgen/src
- confidence: inferred
  path: docs/slice.md
- confidence: inferred
  path: std/net.ignite
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-07T22:11:32Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-07T22:11:32Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-11-08T02:16:34Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"test_result":1}}}
---
# Story: case-study-net-builtins

## Outcome

A server written in Ignite can read a connection's peer address, tell a read that timed out from a peer that closed, and bound a connect by a timeout, on both targets, so the case-study drops its `/proc/self/net/tcp` diff and its timed-read guess.

## Context

- case-study case study (session `case-study`, GitLab `the case study's Ignite candidate`), `docs/research/case-study-gaps.md` G1, G4, G5; `docs/friction.md` rows 60, 63, 64. Timo, 2026-11-07: high priority, "we have to build the case-study".
- G1 (blocking): `crates/ignite-io/src/net.rs` `TcpListener::accept` drops `_peer`; reproducer `docs/research/case-study-gaps/G1.ignite` (E041 `peer_address`).
- G4: `net.rs:388` maps WouldBlock and TimedOut to `Ok(0)`, read as `Eof`; reproducer `G4.ignite` prints `after 205 ms: Eof` and `peer closed: Eof`.
- G5: `TcpStream::connect` (`net.rs` ~359) calls `std::net::TcpStream::connect` with no timeout; reproducer `G5.ignite`.
- `std.net.Endpoint { address, port }` exists for the client side.

## Acceptance

- `TcpStream` has a builtin that returns the peer as `Endpoint` (IPv4 and IPv6 text), on both targets; `G1.ignite` (adapted to the chosen name) runs on both.
- A timed-out read is distinguishable from a close without changing what existing programs do (an additive builtin or result; no existing `match` on `Input` becomes non-exhaustive); `G4.ignite` prints two different outcomes on both targets.
- A connect with a timeout returns an error within the timeout plus 100 ms against `10.255.255.1:80`; existing `connect` unchanged.
- Each builtin is in `docs/slice.md` and the `std/net.ignite` documentation, with inline tests over a pipe or loopback on both targets; `CODEGEN_REVISION` bumped if generation changes.
- `task gate` green.
