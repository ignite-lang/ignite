---
format: aep.planning-md/3
id: story:store-lock-contention
kind: story
status: implemented
title: Store under pipelined load without lock contention
summary: Shard or split the Store lock; futex calls per request toward the handwritten baseline
relations:
- depends_on: story:store-atomic-upsert
scope:
- confidence: cited
  path: crates/ignite-io/src/store.rs
- confidence: cited
  path: crates/ignite-rt/src/store.rs
- confidence: cited
  path: docs/performance
- confidence: cited
  path: docs/slice.md
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-06T15:04:13Z", actor: "human:timo", revision: 7}
- {from: "proposed", to: "active", at: "2026-11-06T15:04:13Z", actor: "human:timo", revision: 8}
- {from: "active", to: "implemented", at: "2026-11-06T18:52:59Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"test_result":1}}}
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

## Acceptance

Scoped 2026-11-06 from main 317955736d: `story:store-atomic-upsert` and `story:store-borrowing-reads` are implemented (`crates/ignite-io/src/store.rs:112-188`); still one `Mutex<BTreeMap>` per store (`:36`), and `update`/`upsert` clone the old value under the lock (`:217`, `:239`). Ember is not in this repository, so the measurement uses a load inside it.

- `update` and `upsert` on the Rust target hold the lock only to take and put back the value: no clone of a stored value while the lock is held (read in the diff; a unit test that a panicking `f` leaves the store usable and the old value in place).
- The map is sharded by id (fixed shard count), and `values()` and `read_values` still yield ascending id order across shards (unit test with ids spread over every shard, and the existing order tests).
- A `#[ignore]` test in `crates/ignite-io` runs 8 threads × 100,000 upserts on 1,000 ids; its wall time and `strace -f -c -e futex` call count, before (main) and after, 3 runs each, recorded in `docs/performance/store-lock/README.md`; the after futex count is lower.
- `docs/slice.md` 11.3 describes the store as landed.
- `task gate` green.

## Out of Scope

- Transactions across keys.
