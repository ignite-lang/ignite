# Store under contended upserts (story:store-lock-contention)

2026-11-06. Before is `00821a2` (`main` at `08452d1` plus the test only: one
`Mutex<BTreeMap>` per store, `update` and `upsert` copy the old value under it). After is the
change to `crates/ignite-io/src/store.rs`: 16 shards by a hash of the id, each a `Mutex` and a
`Condvar`; `update` and `upsert` take the value out under the shard's lock, copy it and run `f`
with no lock held, and lock again only to put the new value back.

Load: `store::tests::contended_upserts_8_threads_1000_ids` (`#[ignore]`, `crates/ignite-io`),
8 threads × 100,000 `upsert_read` calls over 1,000 ids, each value a `Vec<i64>` of 8 whose
first element counts the upserts; the test checks the total (800,000) and prints the wall time
of the 8 threads. Both test binaries were built in one tree with the dev profile
(`CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_INCREMENTAL=0`,
`cargo test -p ignite-io --lib store::`) and copied aside as `ignite_io.before` and
`ignite_io.after`; the runs alternate before and after, one at a time, on 20 cores shared with
other sessions (1-minute load 12.9 to 13.6 throughout).

Commands, per run:

```
./ignite_io.$b store::tests::contended_upserts_8_threads_1000_ids --ignored --exact --nocapture
strace -f -c -e trace=futex -o strace.$b.$i ./ignite_io.$b store::tests::contended_upserts_8_threads_1000_ids --ignored --exact --nocapture
```

## Wall time of the 8 threads, without strace

| run | before s | after s |
|---|---|---|
| 1 | 0.485 | 0.175 |
| 2 | 0.441 | 0.172 |
| 3 | 0.467 | 0.156 |
| median | 0.467 | 0.172 |

## futex calls of the whole test process, under strace

| run | before calls (errors) | after calls (errors) | before s under strace | after s under strace |
|---|---|---|---|---|
| 1 | 426,916 (233,955) | 117,242 (37,606) | 1.585 | 0.595 |
| 2 | 323,440 (174,659) | 120,272 (38,028) | 1.374 | 0.603 |
| 3 | 426,111 (230,556) | 116,358 (38,988) | 1.578 | 0.459 |
| median | 426,111 | 117,242 | 1.578 | 0.595 |

Per upsert: 0.53 futex calls before, 0.15 after (medians over 800,000 upserts). The after
binary takes two lock acquisitions per upsert (take and put back) where the before took one;
its calls are what remains of contention on a shard between two of the eight threads (16
shards), and the waits on a taken id are the `Condvar`'s.

Not measured here: the ember server of the playground session (not in this repository), and
the LLVM target, whose `store_update` and `store_upsert` thunks still run under the shard's
lock (`try_replace_with`, `upsert_with`); they gain the sharding only.
