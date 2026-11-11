---
format: aep.planning-md/3
id: story:llvm-http-connection-closes-its-socket
kind: story
status: active
title: An HTTP connection on the LLVM target closes its socket
relations:
- informed_by: story:binary-size-back
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-11T16:14:42Z", actor: "human:timo", revision: 2, executor: "agent:claude-coordinator", correlation: "wave-11"}
- {from: "proposed", to: "active", at: "2026-11-11T16:14:43Z", actor: "human:timo", revision: 3, executor: "agent:claude-coordinator", correlation: "wave-11"}
---
# Story: an HTTP connection on the LLVM target closes its socket

## Outcome

On the LLVM target a served and closed HTTP connection leaves no open descriptor: `todod` holds as many descriptors after 40 connections as before them, as on the Rust target and as before wave 10.

## Why

Adversary pass 2 on unit size of wave 11 (`review-result:adversary-size-pass-2`, finding 6), `std/protocol/http.ignite:221`: LLVM `todod` never closes an accepted socket. 40 served and closed connections leave 40 open descriptors (7 before, 47 after); the Rust target leaves none (6 and 6) and LLVM `terminald` closes.

Dated by the same adversary: at `115fe70f` (the readiness scheduler's merge) the count is 6 before and 6 after; at `afbf9255` (merge of unit http-app of wave 10) it is 7 and 47. v0.15.0 is older than both, so this is a regression of the release being cut. No test and no row of `docs/quality.md` saw it: row 3.4 (leaked handles after a load run) has no number.

A server that leaks a descriptor per connection stops accepting at the descriptor limit of its process.

## Acceptance

- A test builds `todod` on the LLVM target, serves and closes 40 connections (keep-alive and `Connection: close`, and a connection the client drops mid-request), and finds the descriptor count equal before and after; the same test on the Rust target. It is red before the fix on LLVM.
- The same for a connection an `HttpApp` holds or takes, where the app is the owner: the socket closes when the app lets go of it.
- The cause is named in the commit message: which value holds the socket handle on LLVM and why it was not released.
- Instructions per request and per connection on both targets are not above the tree before the fix beyond run-to-run spread; no new `unsafe`; no file newly over 3,000 lines.
- `task test-ignite` green on both targets; row 1.1 stays 0.

## Scope

- `std/protocol/http.ignite`, `crates/ignite-llvm/src/own.rs`, `crates/ignite-llvm/src/lower.rs`, `crates/ignite-rt` (the release of a stream handle), `crates/ignite/tests/`. Inferred.
