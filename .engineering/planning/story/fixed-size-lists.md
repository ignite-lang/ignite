---
format: aep.planning-md/3
id: story:fixed-size-lists
kind: story
status: proposed
title: '[T; N] fixed-size lists of scalars with constant-length bounds checks'
relations:
- decomposes: epic:native-parity
- depends_on: story:u32-words
revision: 2
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T08:07:15Z", actor: "human:timo", revision: 2}
---
# Story: fixed-size-lists

## Outcome

Fixed-size lists `[T; N]` of scalars are stored inline with a constant length; each index check compares against the constant with a cold failure path, so LLVM drops the provable ones.

## Context

- Epic epic:native-parity; umbrella story:fixed-width-ints; design `docs/fixed-width-ints.md` (PR 120), its sections 2 to 5 and 8.
- codecs (SHA-1 and base64 over 1 MiB) is 2.10x plain Rust on LLVM after the alias work; page faults 5.8x.

## Acceptance

- No call to `ignite_rt_fail_index` in the `-O2` IR of the SHA-1 schedule and round loops over `[U32; 80]` (an LLVM golden, design row 3).
- Fixtures for E058 and E059 and E040 (`[T; 3]` from a 4-literal); codecs LLVM within 1.1x of the reference after the three stories (design row 1).
- The `docs/slice.md` section for the feature written from the design; numbers before and after in the commit body (method of the epic); `task gate` green on both targets.

## Scope

- `crates/ignite-syntax`, `ignite-check`, `ignite-types`, `ignite-eval`, both backends, `std/`. Inferred.
