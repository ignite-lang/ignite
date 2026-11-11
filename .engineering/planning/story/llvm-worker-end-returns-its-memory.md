---
format: aep.planning-md/3
id: story:llvm-worker-end-returns-its-memory
kind: story
status: draft
title: An LLVM worker that ends returns its alternate stack, and a worker's end costs no call more than at v0.15.0
relations:
- informed_by: story:readiness-session-cost
revision: 1
---
# Story: an LLVM worker that ends returns its alternate stack, and a worker's end costs no call more than at v0.15.0

## Outcome

A worker thread of an LLVM-target program unmaps its 64 KiB alternate signal stack when it ends, and the system calls of a first session counted over the worker's whole life are at or below v0.15.0's on both targets.

## Why

Found by the adversary pass on unit session (wave 11, `review-result:adversary-session-pass-1`, findings 6 and 7, both pre-existing):

- `crates/ignite-rt/src/process.rs:151`: 6 workers started, 6 alternate stacks mapped, 0 unmapped, at the unit's base and head. A program whose workers come and go keeps 64 KiB per ended worker.
- `crates/ignite/tests/terminal.rs:839`: the first session including its worker's start and end, by whole-life `strace -f` difference: v0.15.0 LLVM 28 / Rust 34, the unit's base 62 / 65, its head 30 / 35. The extra call is a `munmap` at the worker's end, after the 300 ms rest, outside the window the unit's count (26 and 29) covers.

## Acceptance

- After N workers have ended, N alternate stacks are unmapped (LLVM target). Held by a test that counts `mmap` and `munmap` of the stack's size under `strace -f`, red at today's head.
- The whole-life count of a first session is at most 28 (LLVM) and 34 (Rust), or `docs/slice.md` names the call that stays and why. Held by a count test beside `first_session_calls`.
- `task gate` green; no row of `docs/quality.md` worse.

## Scope

- `crates/ignite-rt/src/process.rs`, `crates/ignite-io/src/sched.rs` (worker end), `crates/ignite/tests/terminal.rs`. Inferred.
