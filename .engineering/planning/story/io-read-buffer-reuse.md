---
format: aep.planning-md/3
id: story:io-read-buffer-reuse
kind: story
status: implemented
title: Reads reuse a per-thread buffer on the Rust target
summary: No 64 KiB allocation and zeroing per read on TCP, pipe and stdin
scope:
- confidence: cited
  path: crates/ignite-io/src
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-04T06:04:08Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-11-04T06:04:08Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-11-04T07:10:10Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---
# Story: io-read-buffer-reuse

## Outcome

A read on a TCP stream, a pipe stream or stdin on the Rust target no longer allocates and zeroes
a 64 KiB buffer per call; it reads into a per-thread scratch buffer and copies out only the bytes
read.

## Context

- Found and implemented by the playground session (2026-11-04, round 7, tree `opt7-syscalls`,
  base `4377844c`): `crates/ignite-io/src/{lib,net,pipe,stdio}.rs`, new `read_input` with a
  thread-local `READ_SCRATCH` replacing `read_buffer` and `input`; reads over 64 KiB still
  allocate.
- Measured on the ember key-value server, Rust target, interleaved A/B over 5 rounds: throughput
  1×1 ×1.160, 16×1 ×1.068, 16×50 ×1.002; server user CPU −16 %. Tournament `04-33-42Z` to
  `06-02-47Z`: heap bytes 98.7 to 34.7 MB, peak RSS 49.8 to 37.6 MiB, churn growth 22.7 to
  0.7 MiB, allocation count unchanged. LLVM unchanged.

## Acceptance

- The test `a_read_returns_exactly_the_bytes_read` and the existing read tests
  (`adv_a_read_returns_every_queued_byte_up_to_maximum`) pass; `task gate` green.

## Out of Scope

- The LLVM target's read path.
