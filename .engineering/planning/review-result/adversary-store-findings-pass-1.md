---
format: aep.planning-md/3
id: review-result:adversary-store-findings-pass-1
kind: review-result
status: active
title: Adversary, store-adversary-findings, pass 1
relations:
- reviews: story:store-adversary-findings
revision: 1
---
```
unit: story:store-adversary-findings, worktree store-findings at edbaaa91 plus my uncommitted test additions
verdict: NEEDS-CHANGE
cases: executed 77→79 (ignite-io lib; store:: 20→22), red 0 on the tree, 2 red on mutants the old suite misses
origin: introduced 2 / pre-existing 1 / undecided 0
wrote-outside-worktree: 2 paths (scratch subtrees, listed in part 6)
needs-coordinator: none
```

1. Diff stat

```
 crates/ignite-io/src/store.rs | 102 ++++++++++++++++++++++++++++++++++++++++++
 1 file changed, 102 insertions(+)
```

`store.rs` holds the implementation and also the test module. All 102 lines are additions inside `mod tests`. No other line of the file changed (the diff is insertions only). Nothing is committed.

2. Cases added (`crates/ignite-io/src/store.rs`, test module)

Both new cases pass on edbaaa91. The code is correct on these points, but nothing in the existing suite would notice if it broke. Each case is red against a mutant that every other test lets through. The mutants were applied to a copy in scratch, never to the worktree.

- **`update_and_upsert_drop_the_old_value_with_no_lock_held_of_their_own_shard`**
  - What it asserts: the old value of an `update` or `upsert` is dropped while no lock is held, as the module doc says. A `DropProbed` value checks, at each drop, whether another thread can read a neighbouring id in the same shard.
  - Mutant M1 (`mut/m1.diff`): `Taken::put_back` drops `old` inside `Shard::put_back`. That makes the slot's Arc share the last one, so `T` is dropped under the shard lock.
  - Result on M1: the other 20 store tests pass. Red output, verbatim:
    ```
    test store::tests::update_and_upsert_drop_the_old_value_with_no_lock_held_of_their_own_shard ... FAILED
    panicked at src/store.rs:1177:9:
    assertion `left == right` failed: values dropped while the task held its shard's lock (4 without)
      left: 2
     right: 0
    test result: FAILED. 21 passed; 1 failed; 1 ignored; 0 measured; 57 filtered out; finished in 2.02s
    ```
- **`read_values_len_counts_no_absent_id_an_upsert_took`**
  - What it asserts: while an `upsert` holds an absent id, `read_values`' `values.len()` equals the number of values it yields, which is 1.
  - Mutant M2 (`mut/m2.diff`): `Values::new` sets `left` from `map.slots.len()`.
  - Result on M2: every other test passes, because `values()` only `collect`s. Red output, verbatim:
    ```
    test store::tests::read_values_len_counts_no_absent_id_an_upsert_took ... FAILED
    panicked at src/store.rs:1206:9:
    assertion `left == right` failed
      left: (2, 1)
     right: (1, 1)
    test result: FAILED. 21 passed; 1 failed; 1 ignored; 0 measured; 57 filtered out; finished in 2.02s
    ```

Order of work: the cases were written and run alone first (`cases-alone.log`, load 13.87, 3 passed). I wrote a third case, a property that values never see an id go back. No mutant turned it red, so I deleted it before the suite ran.

Two more mutants were tried, and the existing suite already catches both:
- M5, the skip loop in `Values::next` removed: caught by `values_len_and_is_empty_see_the_map_before_the_updates_in_progress`.
- M6, `Slot::last` of a taken slot returning `None`: caught by that test and by the two F3/F4 timing tests.

3. Suite runs (`adversary/suite.sh` → `suite.log`, loads 10.3 to 11.1)

- `cargo clippy -p ignite-io --all-targets -- -D warnings`: exit 0
- `cargo fmt --all -- --check`: exit 0
- `cargo test -p ignite-io`:
  - `test result: ok. 79 passed; 0 failed; 1 ignored; ... finished in 2.02s`
  - `test result: ok. 76 passed; 0 failed; 0 ignored; ... finished in 1.59s`
- `cargo test -p ignite-io --lib -- store::`, 10 runs: 10 of 10 printed `test result: ok. 22 passed; 0 failed; 1 ignored; 0 measured; 57 filtered out; finished in 2.02s`. Load ranged from 10.29 to 11.05.
- `cargo test -p ignite --test main -- store`: `test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 572 filtered out; finished in 3.91s`
- `task test-ignite` was not run (see part 5 for how the `T: Sync` bound was checked instead).

4. Findings

- **F-A: missing test (mutant), M1 survives the old suite.** `crates/ignite-io/src/store.rs:223` (`Taken::put_back`).
  - Severity warning, CONFIRMED, introduced.
  - Before this change the taker owned `old` outright. Now, dropping it with no lock held depends on the taker's Arc share being the last one, and only the order inside `put_back` guarantees that. My case pins it.
  - What reaches it: every Rust-target `update` and `upsert`.
- **F-B: missing test (mutant), M2 survives the old suite.** `crates/ignite-io/src/store.rs:260` (`Values::new`'s `left`).
  - Severity note, INFEASIBLE, introduced.
  - The only caller that reads `len()` is `ignite_rt_store_values` (`crates/ignite-rt/src/store.rs`, the `read_values` closure). The LLVM runtime uses only the borrowing forms, so it never has a taken slot. Nothing found reaches the mutant today. The finding is the ExactSizeIterator contract being untested.
- **F-C: slice.md is out of date (contract drift).** `docs/slice.md:1289` still says `Store<T: Clone + Send>`.
  - Severity note, CONFIRMED, pre-existing.
  - The commit message says the docs now name the `Sync` requirement, but only the module doc (`store.rs:28`) does.
  - The bound was already effectively `Send + Sync` for sharing at base. The line now misstates a property the change relies on.
  - Fix: `Store<T: Clone + Send + Sync>`.

Judgement, not a separate finding: the SAFETY comment on `unsafe impl Send/Sync for HeapValue` (`crates/ignite-rt/src/store.rs:56-62`) says "the store's lock orders every access". That no longer holds for `update_read` and `upsert_read`. The runtime never calls them, so this is out of scope.

5. Attacked and could not break

- Torn reads across shards: `lock_every` holds all 16 guards together, so `values` and `len` each see one consistent map.
- Lost wakeups: `waiting` is incremented under the lock, `notify_all` runs on put-back, and the F2 test passed 10 of 10 runs.
- Deadlock: `lock_every` takes the locks in shard order and never waits on a condvar. Single-id waits hold one shard at a time.
- Arc refcounts: no path clones the Arc beyond the slot and the taker. In `Taken::drop`, `Arc::into_inner(...).expect` runs after the slot share is removed. `Taken::copy` borrows only.
- Panics: a panic in `f`, in `T::clone` during `copy`, or in `read` restores the old value; poisoned locks are recovered.
- An absent id taken by an upsert is skipped by `values` and not counted by `len` (the existing test plus my case).
- Remove or put during an update: both wait through `lock_id`.
- The `T: Sync` bound: a scratch crate checked `Store<X>: Send + Sync` for every public handle type of `ignite-io` (TcpStream, TcpListener, PipeStream, Directory, Launcher, ChildProcess, Entropy, Setting, CommandLine, Stdin, Stdout, Store, IoError, Quantity). Exit 0. Generated closures are `Arc<dyn Fn + Send + Sync>`, generic parameters are bounded `Send + Sync`, and `HeapValue` was already `Sync` at base.
- The timing tests F3 and F4 passed 10 of 10 at load about 11.

6. Paths written outside the worktree (all under `/home/timo/.cache/scratch/e18a99fc-f99c-4987-93c9-e5de2080b61a/unit-store-findings/adversary/`)

- `cases-alone.log`, `suite.sh`, `suite.log`
- `synccheck/`: the scratch crate, its 18 MB target and `check.log`
- `mut/`: the copy of `ignite-io` with `store.rs.orig`, `run.sh`, `m1.py.txt` (a note, not Python), `m1.diff`, `m2.diff`, `m5.diff`, `m6.diff`, `m1.log`, `m2.log`, `m5.log`, `m6.log`, and a 45 MB target

```findings
[
  {"file": "crates/ignite-io/src/store.rs", "line": 223, "category": "mutant", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "No existing test catches dropping the old value under the shard lock (Taken::put_back letting the slot's Arc share be the last); update_and_upsert_drop_the_old_value_with_no_lock_held_of_their_own_shard does."},
  {"file": "crates/ignite-io/src/store.rs", "line": 260, "category": "mutant", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "Values::left counting a taken absent id survives the old suite; only ignite_rt_store_values reads len() and the LLVM runtime never takes a slot."},
  {"file": "docs/slice.md", "line": 1289, "category": "contract-drift", "severity": "note", "verdict": "CONFIRMED", "origin": "pre-existing", "message": "slice.md still states Store<T: Clone + Send> although a shared store needs T: Sync, which the commit says the docs now name."}
]
```
