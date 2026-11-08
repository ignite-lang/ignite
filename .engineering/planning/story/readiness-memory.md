---
format: aep.planning-md/3
id: story:readiness-memory
kind: story
status: proposed
title: Per-worker read buffers and stack pools close the RSS gap
relations:
- decomposes: epic:native-parity
- decomposes: story:readiness-io
- depends_on: story:readiness-scheduler
scope:
- confidence: cited
  path: crates/ignite-io/src/sched.rs
- confidence: cited
  path: crates/ignite-rt/src/arena.rs
- confidence: cited
  path: crates/ignite-rt/src/net.rs
- confidence: inferred
  path: docs/readiness-io.md
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T08:45:33Z", actor: "human:timo", revision: 4}
---
# Story: readiness-memory

## Outcome

The read scratch buffer, the spare arena chunk and free task stacks are kept per worker instead of per task, and a returned stack is trimmed above 64 KiB, so todo get-64's peak RSS comes within 1.1x of plain Rust; the memory per idle task is measured, which is the number that decides whether stackless lowering is ever worth choosing. Phase R3 of `docs/readiness-io.md`.

## Context

- Design: `docs/readiness-io.md` sections 4.6 (stacks), 4.11 (memory), 3.4 (what to measure before stackless lowering), 7 (R3), risk K3, Q5.
- Builds on `story:readiness-scheduler`.

## Acceptance

- `READ_BUFFER` in `crates/ignite-rt/src/net.rs` per worker; the spare arena chunk a per-worker pool; a worker's free-stack pool `madvise(MADV_DONTNEED)`s a returned stack above 64 KiB touched.
- `docs/readiness-io.md` section 6 row 5: get-64 peak RSS at most 1.1x plain Rust; row 7: RSS of 10,000 idle Telnet sessions recorded with the touched stack per idle task, no target; both in the commit body and appended to `docs/readiness-io.md` section 3.4 as the measurement it names.
- `task gate` green on both targets, counts and exit in the PR body.

## Scope

- `crates/ignite-rt/src/{net,arena}.rs`, `crates/ignite-io/src/sched.rs`, `docs/readiness-io.md` (the measurement). Code paths cited from `docs/readiness-io.md` section 8; the document inferred from section 3.4.
