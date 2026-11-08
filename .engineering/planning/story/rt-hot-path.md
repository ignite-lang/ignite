---
format: aep.planning-md/3
id: story:rt-hot-path
kind: story
status: implemented
title: Runtime hot calls without guard overhead; smaller buffers; kept large blocks
relations:
- decomposes: epic:native-parity
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T03:59:05Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-08T03:59:07Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-11-08T08:52:50Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
# Story: rt-hot-path

## Outcome

`ignite_rt_arena_state` is a plain thread-local read and the panic hook is installed once at start; stream handle lookup needs no mutex or refcount churn per operation; the HTTP read size and kept buffers are 8 to 16 KiB and an idle worker keeps no spare 64 KiB arena chunk; the arena keeps freed large blocks; so todo and terminal spend less CPU and memory per reply.

## Context

- Epic epic:native-parity (diagnosis and evidence in its body).

## Acceptance

- Panic containment (`contain::tests::every_entry_is_guarded`, quality row 3.5) still holds for every entry that can panic.
- get-64 user instructions per reply down at least 15 %; get-64 peak RSS within 1.3x; codecs page faults within 3x of the reference.
- Measured before and after locally with `perf stat` cost per reply against plain Rust (the method in the epic); the numbers go in the commit body; `task gate` green.

## Scope

- `crates/ignite-rt/src/arena.rs`, `contain.rs`, `net.rs`, `crates/ignite-io/src/report.rs`, `std/protocol/http.ignite`. Inferred.
