---
format: aep.planning-md/3
id: story:llvm-store-update-off-lock
kind: story
status: draft
title: LLVM Store update runs f off the shard lock
relations:
- informed_by: review-result:adversary-store-findings-pass-2
revision: 2
---
# Story: llvm-store-update-off-lock

## Outcome

On the LLVM target, `values` returns within 500 ms while tasks update ids of one shard, as on the Rust target: an update's `f` runs with no shard lock held.

## Context

- Found by the second adversary of `story:store-adversary-findings` (`review-result:adversary-store-findings-pass-2`, 2026-11-07); out of that story's scope ("the LLVM target's callbacks under the shard lock").
- Site: `crates/ignite-rt/src/store.rs:218`: `ignite_rt_store_update` uses `try_replace_with`, so the generated thunk runs `f` under the shard lock, and `read_values` competes with every writer for the mutex.
- Measured: `fixtures/tests/store_load.ignite:52` (4 tasks with a slow pure `f` on ids 8 and 21, shard 15; 300 `values` calls), LLVM red 18 of 20 on base `6fa8da18`, 19 of 20 with the Rust-side fix; Rust 46 of 46 green. The case and its wrapper are on `fix/store-adversary-findings` at `0dcc876f` (main after wave 4): `crates/ignite/tests/fixtures/tests/store_load.ignite` and `ignite_test::store_values_under_updates_of_one_shard_on_both_targets`, `#[ignore]`d for this story; un-ignore it.
- What reaches it: any LLVM-built program (the default target) that lists values while tasks update ids of one shard; whether todod's fast `f` reaches 500 ms is not measured.

## Acceptance

- `store_load.ignite` and `ignite_test::store_values_under_updates_of_one_shard_on_both_targets` pass on both targets, 20 of 20 runs at load over 10.
- `docs/slice.md` and `docs/llvm.md` state where the LLVM thunk runs.
- `task gate` green.
