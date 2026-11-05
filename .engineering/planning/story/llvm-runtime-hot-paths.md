---
format: aep.planning-md/3
id: story:llvm-runtime-hot-paths
kind: story
status: active
title: LLVM runtime hot paths for text servers
summary: Text conversions without UTF-8 pass or heap, inlined literal text equality
scope:
- confidence: inferred
  path: crates/ignite-llvm/src
- confidence: inferred
  path: crates/ignite-rt/src
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-04T07:02:51Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-11-04T07:02:51Z", actor: "human:timo", revision: 4}
---
# Story: llvm-runtime-hot-paths

## Outcome

The LLVM target's runtime and lowering spend less CPU per request on a text protocol server:
text conversions without a UTF-8 pass or heap allocation, and text equality against literal
match arms inlined.

## Context

- Found and implemented by the playground session (2026-11-04), ember key-value server, LLVM
  target, profiled with perf:
  - round 6 (tree `ignite-opt6-llvm-cpu`, base `90de54c2`): `crates/ignite-rt/src/text.rs`
    `ignite_rt_text_from_bytes_lossy` copies ASCII straight to the arena,
    `ignite_rt_int_parse` parses digits from bytes, `ignite_rt_int_to_text` formats into a stack
    buffer. Interleaved A/B 16×50, 5 rounds, load 11–15: 716,524 to 831,600 ops/s, median ratio
    1.133, every round at least 1.115; heap allocations 172,622 to 156,645. PR 18 (`857f204f`)
    added `parse_int` to `ignite-io` and changed the same file; the two parse paths become one.
  - next: inlined `Text`/`Bytes` equality for literal match arms (glue `eq_function`, 3 sites in
    `crates/ignite-llvm/src/lower.rs`), +5.1 % measured alone.
- Profile before round 6: kernel time 52 % of server CPU, 1.16 voluntary context switches per op
  (attribution to the Store lock inferred), `lock_contended` 14 %, `copy_to_heap.ember.Bucket`
  8 % with about 4 mallocs per copy (`story:store-lock-contention`).

## Acceptance

- Each step: `Int.parse`, `to_text` and lossy decoding give the same results as before on both
  targets (inline tests, run on the base and after), the playground's A/B cited, `task gate`
  green.

## Out of Scope

- The Store lock (`story:store-lock-contention`).
