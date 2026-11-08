---
format: aep.planning-md/3
id: story:leak-census
kind: story
status: active
title: 'Runtime census and soak: seeded leaks and G30 fail a test'
summary: Per-kind live counters and heap bytes on both targets, std.runtime census, settle and soak
relations:
- decomposes: epic:leak-guard
scope:
- confidence: inferred
  path: crates/ignite-check/src/lib.rs
- confidence: inferred
  path: crates/ignite-check/tests/layering.rs
- confidence: inferred
  path: crates/ignite-io/src/census.rs
- confidence: inferred
  path: crates/ignite-io/src/channel.rs
- confidence: inferred
  path: crates/ignite-io/src/child.rs
- confidence: inferred
  path: crates/ignite-io/src/entropy.rs
- confidence: inferred
  path: crates/ignite-io/src/fs.rs
- confidence: inferred
  path: crates/ignite-io/src/lib.rs
- confidence: inferred
  path: crates/ignite-io/src/net.rs
- confidence: inferred
  path: crates/ignite-io/src/pipe.rs
- confidence: inferred
  path: crates/ignite-io/src/store.rs
- confidence: inferred
  path: crates/ignite-io/src/task.rs
- confidence: inferred
  path: crates/ignite-llvm/src/glue.rs
- confidence: inferred
  path: crates/ignite-llvm/src/ir.rs
- confidence: inferred
  path: crates/ignite-llvm/src/lower.rs
- confidence: inferred
  path: crates/ignite-rt/src/arena.rs
- confidence: inferred
  path: crates/ignite-rt/src/census.rs
- confidence: inferred
  path: crates/ignite-rt/src/channel.rs
- confidence: inferred
  path: crates/ignite-rt/src/lib.rs
- confidence: inferred
  path: crates/ignite-rt/src/net.rs
- confidence: inferred
  path: crates/ignite-rt/src/store.rs
- confidence: inferred
  path: crates/ignite-rustgen/src/lib.rs
- confidence: inferred
  path: crates/ignite-types/src/builtins.rs
- confidence: inferred
  path: crates/ignite-types/src/lib.rs
- confidence: inferred
  path: crates/ignite-types/src/typer.rs
- confidence: inferred
  path: crates/ignite/src/apply/plan.rs
- confidence: inferred
  path: crates/ignite/tests/fixtures/leak
- confidence: inferred
  path: crates/ignite/tests/main.rs
- confidence: inferred
  path: docs/layering.md
- confidence: inferred
  path: docs/slice.md
- confidence: inferred
  path: std/runtime.ignite
- confidence: inferred
  path: std/sync.ignite
revision: 11
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T15:04:30Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-11-08T15:04:30Z", actor: "human:timo", revision: 4}
---
# Story: leak-census

## Outcome

A test can count the live runtime objects of its process per kind and the heap bytes it holds, on both targets, and can soak a body: `assert soak(2000, body).growth == []` fails, naming the kind that grew, when the body leaks a counted object or bytes. Its first failing tests are G30's: a channel end bound a second time or kept in a record field fails on `main` on LLVM by the live handle count; and a deliberately leaked object of every compared kind fails it independently of any real bug.

## Context

- Design: `docs/leak-guard.md` sections 1.1 and 1.2 (G30's cause and the leak class), 5.1 (the census, its kinds, counted heap bytes), 5.2 (`std.runtime`), 6.1 (the G30 soaks), 7 (acceptance), Q4, Q6, Q7, Q10, Q11.
- Counters live in `crates/ignite-io` (one census, both targets: tasks, channels, listeners, streams, pipes, children, directories, capabilities, stores, workers); `crates/ignite-rt` registers its LLVM-only kinds (filled handles, handle slots, heap copies, arena and pooled bytes).
- Heap bytes: a counting global allocator with per-thread counters; Rust target: `#[global_allocator]` in the generated program and test crates; LLVM: in `ignite-rt`, and generated code calls `ignite_rt_heap_alloc` and `ignite_rt_heap_free` instead of `@malloc` and `@free`. A generation change: `apply::plan::CODEGEN_REVISION` moves once.
- G30 (2026-11-08): a channel end moved out of its plain `let (tx, rx)` binding (a second annotated binding, an annotated tuple, a record field, a `Store`) is never released on LLVM (`own.rs`); the three lines first reported do not leak. Cause, forms, numbers and the reproducer's program text: `story:channel-release-on-close`.
- Cost: not measured locally (AGENTS.md); the next release's `task quality-remote` measures it.

## Acceptance

- First failing case, G30: the soak tests "a channel end bound a second time is released (G30)" and "a channel end kept in a record field is released (G30)" in `std/sync.ignite`, the case study's `alias` and `field` forms run inline 2,000 times each (`docs/leak-guard.md` 6.1, program text in `story:channel-release-on-close`), fail on `main` on the LLVM target with `growth` naming `handles` and `channels`, and pass on the Rust target. That red run is recorded against this story (`test_result` evidence with the run's output); the tests reach `main` only together with `story:channel-release-on-close`, which turns them green, in one wave with this story first.
- Every kind counts: an inline test in `std/runtime.ignite` creates one object of each kind on each target where the kind exists (a task, a channel, a listener, a stream, a pipe end, a child, a directory, a capability, a store; on LLVM also a handle and a heap copy) and asserts that its `Runtime.census()` field rises by exactly 1 and, except `stores` (resident), returns to its value before once the object is released; `heap_bytes` rises by at least 1 MiB while a 1 MiB value is held and returns within 64 KiB of its value before once it is dropped; on LLVM `arena_bytes` rises while a spawned task holds a 1 MiB value and returns within 64 KiB once the task has ended.
- Seeded leaks, red independently of any real bug: debug builds of `ignite-io` and `ignite-rt` read `IGNITE_DEBUG_LEAK=<kind>` and then skip one release per object of that kind, for every compared kind (`task` never decrements, `channel`, `listener`, `stream`, `pipe`, `child`, `directory`, `capability` on both targets; `handle`, `heap_copy` and `arena`, a task's arena block kept at its end, on LLVM; `bytes`: 370 B per closed channel holding no counted object, both targets). A fixture under `crates/ignite/tests/fixtures/leak/` holds one soak test per kind and one plain test that creates and drops one channel (the plain test is the one `story:leak-check-tests` checks); every soak test passes without the variable and fails with its value on each target where the kind exists, its `growth` naming that kind (`bytes`: `heap_bytes`, `arena`: `arena_bytes`).
- `std.runtime` (base layer; `docs/layering.md` and `layering::` change together) provides `Runtime.census()`, `Runtime.settle(tasks, within)`, `soak(runs, body)` with warm-up `max(10, runs / 10)`, counts exact and bytes within 64 KiB, and `Census`, `Growth`, `Soak`; a substrate builtin has no value in the evaluator.
- `docs/slice.md` gains the section for the census and `std.runtime`; `task gate` green at the wave's merge, after the fix.

## Scope

- `crates/ignite-io/src/census.rs` (new); counters in `crates/ignite-io/src/{lib,task,channel,net,pipe,child,fs,store,entropy}.rs`. Inferred.
- `crates/ignite-rt/src/census.rs` (new); `crates/ignite-rt/src/{lib,net,arena,store,channel}.rs`: LLVM kinds, the counting allocator, `ignite_rt_heap_alloc` and `ignite_rt_heap_free`. Inferred.
- `crates/ignite-llvm/src/{glue,ir,lower}.rs`, `crates/ignite-rustgen/src/lib.rs`, `crates/ignite-check/src/lib.rs` and `crates/ignite-types/src/{lib,builtins,typer}.rs` (the builtin tables): lowering and builtins. Inferred.
- `std/runtime.ignite` (new), `std/sync.ignite`, `docs/layering.md`, `crates/ignite-check/tests/layering.rs`, `crates/ignite/tests/fixtures/leak/`, `crates/ignite/tests/main.rs`, `crates/ignite/src/apply/plan.rs` (`CODEGEN_REVISION`), `docs/slice.md`. Inferred.
