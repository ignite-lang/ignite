---
format: aep.planning-md/3
id: story:spawn-without-handshake
kind: story
status: active
title: Spawning a task does not wait for the worker to copy its environment
relations:
- decomposes: epic:native-parity
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T03:59:06Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-08T06:03:41Z", actor: "human:timo", revision: 3}
---
# Story: spawn-without-handshake

## Outcome

On the LLVM target `spawn` hands the task a heap copy of its environment and returns without waiting on a channel, so accepting connections costs as few futex calls as plain Rust.

## Context

- Epic epic:native-parity (diagnosis and evidence in its body).

## Acceptance

- futex calls per connection within 1.3x of the reference (`strace -c`); connect-8 throughput recorded on a quiet host.
- Measured before and after locally with `perf stat` cost per reply against plain Rust (the method in the epic); the numbers go in the commit body; `task gate` green.

## Scope

- `crates/ignite-rt/src/task.rs`, `crates/ignite-llvm` (spawn lowering). Inferred.
