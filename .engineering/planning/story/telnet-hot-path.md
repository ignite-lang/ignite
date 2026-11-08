---
format: aep.planning-md/3
id: story:telnet-hot-path
kind: story
status: implemented
title: Telnet encodes the prompt once and decodes without per-byte tuples; fair urgent-data cost
relations:
- decomposes: epic:native-parity
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T03:59:06Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-08T03:59:07Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-11-08T08:52:53Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
# Story: telnet-hot-path

## Outcome

`std.protocol.telnet` encodes a session's prompt once and decodes input without a per-byte tuple carrying bytes, and the terminal benchmark's urgent-data handling costs the same on both sides (the substrate checks urgent data without a syscall per read, or the reference does the same poll), so terminal CPU per reply is within 1.1x.

## Context

- Epic epic:native-parity (diagnosis and evidence in its body).

## Acceptance

- The Telnet conformance claims still hold (`ignite conformance`).
- terminal rr-1 and rr-64 CPU per reply within 1.1x of the reference.
- Measured before and after locally with `perf stat` cost per reply against plain Rust (the method in the epic); the numbers go in the commit body; `task gate` green.

## Scope

- `std/protocol/telnet.ignite`, `crates/ignite-io/src/net.rs` or `bench/native/rust/terminal/`. Inferred.
