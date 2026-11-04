---
format: aep.planning-md/3
id: story:store-lock-contention
kind: story
status: draft
title: Store under pipelined load without lock contention
summary: Shard or split the Store lock; futex calls per request toward the handwritten baseline
relations:
- depends_on: story:store-atomic-upsert
scope:
- confidence: inferred
  path: crates/ignite-io/src/store.rs
revision: 4
---
# Story: store-lock-contention

## Outcome

Pipelined load on a `Store` no longer spends its time contending on one mutex: futex calls per
request on the ember server fall toward the handwritten Rust baseline.

## Context

- Measured by the playground session (2026-11-04, after round 7), ember key-value server, Rust
  target, pipeline 50: futex calls per request 0.75 against 0.07 for the handwritten Rust server;
  `lock_contended` 11.9 % of user CPU (strace `opt7/b-s50.strace`, perf `opt7/b-p50.perf.data` in
  that session's scratch directory).
- Causes named there: one `Mutex<BTreeMap>` per store (`ignite_io::Store`, `docs/slice.md` 11.3);
  INCR takes the lock twice (`get`, then `update`); `update` clones the value twice while holding
  the lock. `story:store-atomic-upsert` turns INCR into one lock; `story:store-borrowing-reads`
  changes the copies.
- Copies under the lock: after round 10 (passed closures), 48,000 blocks remain at ember's INCR
  update site, inferred to be `f(old.clone())` in `ignite_io::Store::update`
  (`crates/ignite-io/src/store.rs:175` at `88cae227`); on LLVM `copy_to_heap.ember.Bucket` is
  8 % of CPU with about 4 mallocs per copy (`story:llvm-runtime-hot-paths`). Fewer copies shorten
  the time the lock is held.
- LLVM memory (playground session, round 14, massif and dhat of ember): each upsert makes 4 mallocs
  in `copy_to_heap.ember.Bucket`; after FLUSHDB RSS is 18.5 MiB against 3.8 MiB for the
  handwritten Rust server. FLUSHDB's `db.values()` deep-copies the keyspace
  (`ignite_rt_store_values` → `copy_from_heap.ember.Bucket`), 10.7 MB of a 24.3 MB heap peak; a
  lending `values` on LLVM (`crates/ignite-io/src/store.rs`, `crates/ignite-llvm/src/lower.rs`)
  removes it.
- Options: shard the map by id, a reader-writer lock for reads, clones outside the lock.

## Acceptance

- futex calls per request and `lock_contended` share measured before and after on the same load,
  recorded in `docs/performance/`; `values()` keeps ascending id order; `task gate` green.

## Out of Scope

- Transactions across keys.
