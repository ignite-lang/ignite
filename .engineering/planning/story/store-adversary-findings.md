---
format: aep.planning-md/3
id: story:store-adversary-findings
kind: story
status: draft
title: 'Sharded Store: tests that catch its mutants, values under write load'
revision: 1
---
# Story: store-adversary-findings

## Outcome

The sharded Store of PR 74 has tests that fail on its mutants, `values`/`len` return under overlapping writes of one shard, and its doc states what holds.

## Context

- Adversary review of PR 74 (2026-11-07), failing tests on branch `test/adv-store-lock-contention` at `e9fb7135` (`crates/ignite-io/src/store.rs` test module only).
- F1 (introduced): the acceptance test `update_and_upsert_copy_no_value_while_holding_the_lock` probes id 2 in another shard than id 1 (`store.rs:758`), so a mutant cloning under the lock passes every store test; the branch's `..._of_their_own_shard` test catches it.
- F2 (introduced): `notify_all` → `notify_one` (`store.rs:150`) survives every existing test; the branch's wakeup test catches it.
- F3 (pre-existing, also red on base): `values` and `len` wait 1.9 s and longer while two updates of one shard overlap: `lock_whole` (`store.rs:136`) waits for a moment with no taken slot. Reached by a program that lists values under write load.
- F4 (introduced doc): the module doc (`store.rs:6`) says tasks on different shards do not wait for each other; a pending `values` holding shards 0-14 stalls a `get` of shard 0 for 1.8 s (`store.rs:285`).

## Acceptance

- The four tests of `test/adv-store-lock-contention` are on main and pass; the old probe test is replaced by the same-shard one.
- `values_returns_while_two_updates_of_one_shard_overlap` passes: `values`/`len` do not starve under overlapping takes (e.g. read a taken slot's last put value, or a writer-preferring gate).
- `a_pending_values_does_not_stop_a_read_of_an_idle_shard` passes, or the doc states the wait and the test is replaced by one asserting the documented bound.
- `task gate` green.

## Out of Scope

- The LLVM target's callbacks under the shard lock.
