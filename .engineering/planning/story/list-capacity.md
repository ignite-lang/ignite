---
format: aep.planning-md/3
id: story:list-capacity
kind: story
status: active
title: List.with_capacity and Bytes.with_capacity
relations:
- decomposes: epic:native-parity
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T08:07:15Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-08T08:07:15Z", actor: "human:timo", revision: 3}
---
# Story: list-capacity

## Outcome

`List.with_capacity(n)` and `Bytes.with_capacity(n)` create an empty list or bytes with room for `n` elements; only speed is observable, the evaluator ignores the hint; std's codecs and HTTP rendering use it.

## Context

- Epic epic:native-parity; umbrella story:fixed-width-ints; design `docs/fixed-width-ints.md` (PR 120), its sections 2 to 5 and 8.
- codecs (SHA-1 and base64 over 1 MiB) is 2.10x plain Rust on LLVM after the alias work; page faults 5.8x.

## Acceptance

- `len()` is 0, `push` beyond `n` grows, `n < 0` is 0: inline tests on both targets and in the evaluator.
- codecs page faults at most 1.5x the reference (design row 2); Rust-target todo get allocations per reply recorded.
- The `docs/slice.md` section for the feature written from the design; numbers before and after in the commit body (method of the epic); `task gate` green on both targets.

## Scope

- `crates/ignite-check`, `ignite-types`, `ignite-eval` builtins, `crates/ignite-llvm`, `crates/ignite-rustgen`, `crates/ignite-rt`, `std/`. Inferred.
