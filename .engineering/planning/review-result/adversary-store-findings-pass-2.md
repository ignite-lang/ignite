---
format: aep.planning-md/3
id: review-result:adversary-store-findings-pass-2
kind: review-result
status: active
title: Adversary, store-adversary-findings, pass 2
relations:
- reviews: story:store-adversary-findings
revision: 1
---
The correction holds. The one break I found is on the LLVM path, which the story puts out of scope, and the same case is red on the base commit.

```
unit: story:store-adversary-findings, worktree store-findings at 23a0561d plus my uncommitted test additions
verdict: INFEASIBLE
cases: executed ignite-io lib 79→80 (store:: 22→23); ignite e2e `store ignite_test::` 27→28, red 1
origin: introduced 1 / pre-existing 1 / undecided 0
wrote-outside-worktree: 1 directory tree (part 6)
needs-coordinator: whether store_load.ignite stays in the suite: it is red on LLVM under load, and that behaviour is out of scope for this story
```

## 1. Diff stat

```
 crates/ignite-io/src/store.rs      | 24 ++++++++++++++++++++++++
 crates/ignite/tests/ignite_test.rs | 18 ++++++++++++++++++
 2 files changed, 42 insertions(+)
?? crates/ignite/tests/fixtures/tests/store_load.ignite
```

- `store.rs` holds both the implementation and its tests. All 24 lines are insertions inside `mod tests`, after the last test; no implementation line changed.
- The other two paths are test files. Nothing is committed.

## 2. Cases added

**`store::tests::is_empty_while_an_upsert_holds_only_an_absent_id`** (`crates/ignite-io/src/store.rs`, end of the test module)
- Asserts: while an upsert holds an absent id in an empty store, `(is_empty, len, values)` is `(true, 0, [])`.
- Alone on the tree: green (`1 passed … 80 filtered out`).
- Against mutant m7 (`store.rs:376`, `map.len() == 0` changed to `map.slots.is_empty()`, applied to a scratch copy), only this case goes red. All 22 other store tests pass:
  ```
  test store::tests::is_empty_while_an_upsert_holds_only_an_absent_id ... FAILED
  panicked at src/store.rs:1238:9:
  assertion `left == right` failed
    left: (false, 0, [])
   right: (true, 0, [])
  test result: FAILED. 22 passed; 1 failed; 1 ignored; 0 measured; 57 filtered out; finished in 2.02s
  ```

**`fixtures/tests/store_load.ignite`, plus `ignite_test::store_values_under_updates_of_one_shard_on_both_targets`**
- Setup: 4 tasks run `s.update` with a slow pure `f` on ids 8 and 21, which are both in shard 15. The test calls `s.values()` 300 times meanwhile.
- Asserts, on every list:
  - each call returns within 500 ms (the bound the Rust test uses);
  - the list holds 3 ids;
  - no value goes back.
- At the end it asserts the store is `[800, 0, 800]`.
- First run alone (`target/debug/ignite test`, load 17): green on both targets. The Rust wrapper alone: green.
- First red came in the suite (part 3). Reproduced with full output, LLVM, load 25:
  ```
  test fixtures.store_load/values under updates of one shard: every id, values grow, no long wait, no lost write ... FAILED
  ---- fixtures.store_load/values under updates of one shard: every id, values grow, no long wait, no lost write stderr ----
  error: assert failed at crates/ignite/tests/fixtures/tests/store_load.ignite:52:5: Clock.monotonic() - before < 500 ms
    message: values waited for the writers of its shard
  test result: FAILED. 0 passed; 1 failed; 0 filtered out; finished in 1.29s
  ```
- 40 runs per target, both targets at once, load about 23 to 25: LLVM 6 ok and 34 red; Rust 40 ok.
- Base (6fa8da18) against head, 20 interleaved rounds on LLVM, load 13 to 14: base red 18 of 20, head red 19 of 20. Rust, 6 rounds: both 6 of 6 ok.

## 3. Suite run

`adversary2/suite.sh`, run after both cases existed:

| step | result |
|---|---|
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy -p ignite-io -p ignite --all-targets -- -D warnings` | exit 0 |
| `cargo test -p ignite-io` | lib `80 passed; 0 failed; 1 ignored`; tests `76 passed; 0 failed` |
| `cargo test -p ignite-io --lib -- store::`, 5 runs at load 14 to 15 | 5 of 5 `23 passed; 0 failed; 1 ignored`; pass 1's two cases are inside these |
| `cargo test -p ignite --test main -- store ignite_test::` | `27 passed; 1 failed … finished in 572.87s`, exit 101. The failure is my LLVM case (assertion as in part 2). |
| same command again, `suite-e2e.log` | `28 passed; 0 failed … 10.03s`, exit 0 |
| `cargo test -p ignite-rt --lib -- store` | `2 passed` |

## 4. Findings (they cover worktree 23a0561d plus my additions)

- **F1: on LLVM, `values` waits more than 500 ms while tasks update one shard.** `crates/ignite-rt/src/store.rs:218`.
  - Category concurrency, severity warning, INFEASIBLE, pre-existing.
  - Measured: `store_load.ignite:52`. Base 18 of 20 red, head 19 of 20 red.
  - Cause: `ignite_rt_store_update` uses `try_replace_with`, so the generated thunk runs `f` under the shard lock. `read_values` then has to win shard 15's mutex against 4 writers that release and take it again at once.
  - The Rust target now passes the same case 40 of 40 times.
  - INFEASIBLE because the story lists "The LLVM target's callbacks under the shard lock" as out of scope. It is not contract drift: `docs/slice.md` says the LLVM thunk runs under the lock.
  - What reaches it: any program built for LLVM, the default target, that lists values while tasks update ids of one shard. I built a deliberately slow `f`; whether todod's fast `f` reaches 500 ms was not measured.
- **F2: mutant m7 (`is_empty` counting the shard's raw slots) survives the committed suite.** `crates/ignite-io/src/store.rs:376`.
  - Category mutant, severity note, INFEASIBLE, introduced. My case catches it.
  - What reaches it: nothing found. `is_empty` and `len` are not Ignite methods, and no caller exists in rustgen or ignite-rt.
- **F3: mutant m8 (removing `self.left -= 1` in `Values::next`) survives the suite.** `crates/ignite-io/src/store.rs:288`.
  - Category mutant, severity note, INFEASIBLE, introduced. No case written.
  - What reaches it: nothing found. The only reader of `len()` is `ignite_rt_store_values`, which reads it once before iterating. The exact-size length after a partial iteration is untested.

## 5. Attacked and could not break

- **The debug_assert in release:** it compiles out and changes nothing. The order inside `put_back` stays the guarantee, and pass 1's drop test still pins it.
- **The assertion firing on a legitimate path:** the only `Arc::clone` of a value is the slot's share (`store.rs:198`). `Values` yields `&T` through `Slot::last`, under the locks only, so no Arc clone ever leaves a lock. Every other path waits through `lock_id`, so `put_back` always finds the slot's share present and drops it under the lock. It did not fire in 5 store runs or the e2e runs.
- **The ignite-rt SAFETY comments:** true as now worded. Every drop of a `HeapValue` (by `put`, `remove`, `try_replace_with` or `upsert_with`) happens after the value left the map under its lock. `Store<T>: Sync` needs `T: Send + Sync`, because the slot holds an `Arc<T>`.
- **The `Store<T: Clone + Send + Sync>` line in `docs/slice.md`:** matches the code.
- **Mutant m9 (`waiting > 0` changed to `> 1`):** caught; three store tests hang (killed after more than 60 s).
- **Pass 1's cases:** green in all store runs.
- **Rust target under load:** `store_load.ignite` passed 46 of 46 runs. The case cannot tell base from head on Rust (base also passed 6 of 6), so it adds no guard for the original values-starvation defect (F3 of the story) on the Rust target.

## 6. Paths written outside the worktree

All under `/home/timo/.cache/scratch/e18a99fc-f99c-4987-93c9-e5de2080b61a/unit-store-findings/adversary2/`, 354 MB after I deleted `base-target/` (698 MB):
- `mut/`: a copy of `ignite-io` with `store.rs.orig`, its `target/`, `run.sh`, and `m7`, `m8`, `m9` `.diff` and `.log`.
- `base/`: a `git archive` of 6fa8da18.
- `base-cargo/`, `head-cargo/`, `il-base-llvm/`, `il-head-llvm/`, `il-base-rust/`, `il-head-rust/`, `out-llvm/`, `out-rust/`: build outputs of the case runs.
- Scripts: `suite.sh`, `loop.sh`, `interleave.sh`.
- Logs: `suite.log`, `suite-e2e.log`, `rerun.log`, `rust-case-alone.log`, `ignite-case-alone.log`, `loop-llvm.log`, `loop-rust.log`, `interleave-llvm.log`, `interleave-rust.log`, `base-build.log`.

## 7. Findings block

```findings
[
  {"file": "crates/ignite-rt/src/store.rs", "line": 218, "category": "concurrency", "severity": "warning", "verdict": "INFEASIBLE", "origin": "pre-existing", "message": "On the LLVM target store_load.ignite:52 sees values wait over 500 ms while 4 tasks update two ids of one shard (base 18/20 red, head 19/20), because the update thunk runs under the shard lock; out of the story's scope."},
  {"file": "crates/ignite-io/src/store.rs", "line": 376, "category": "mutant", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "is_empty counting the shard's raw slots survives every committed test; is_empty_while_an_upsert_holds_only_an_absent_id catches it, and no caller of is_empty exists."},
  {"file": "crates/ignite-io/src/store.rs", "line": 288, "category": "mutant", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "Dropping the left decrement in Values::next survives the suite; the only len() reader reads it before iterating."}
]
```
