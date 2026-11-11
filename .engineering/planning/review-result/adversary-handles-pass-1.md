---
format: aep.planning-md/3
id: review-result:adversary-handles-pass-1
kind: review-result
status: active
title: Adversary pass 1, unit handles (wave 11)
relations:
- reviews: story:llvm-release-handle-temporaries
revision: 1
---
red

Adversary pass 1 on unit handles, `wave/11-handles` at `041ee31c`, test commit `06be2bea`.

verdict: red; cases executed 1652→1654, red 2; origin: introduced 3, pre-existing 2.

Findings:
1. `crates/ignite/tests/soak/temporaries_attack.ignite:214` (root cause `crates/ignite-llvm/src/region.rs:119`), CONFIRMED, introduced, warning. A fresh end passed to a callee that rebinds it through a call (`let u = same(t)`) and spawns a task with `u` is released by the new owner scope while the task uses it ("7 is not a channel sender"). `region::moved_to_task` treats any call as fresh, so the handle moves to the task without an added owner. The case ran (leaking) at `f58a7733`. Proposed: `region::fresh_value` decides freshness with `own::produces`, so only a fn that `returns_fresh` counts.
2. `crates/ignite-llvm/src/lower.rs:1603`, CONFIRMED, introduced, warning. Four mutants survive the unit's tests: release the copy before the callee runs (`lower.rs:1603`), drop the callee's result check (`own.rs:143`), drop the temporaries of `put`, `send`, `try_send` (`own.rs:127`), drop role calls (`own.rs:126`). In every unit case the store still holds its own owner while the temporary is used. The adversary's `temporaries_attack.ignite` (10 cases) kills all four.
3. `docs/slice.md:10147`, CONFIRMED, introduced, note. 30.3 drops the known-gap text, but five temporary forms are still never released on LLVM and are listed nowhere.
4. `crates/ignite/tests/soak/temporaries_unreleased.ignite:78`, CONFIRMED, pre-existing, warning. Five store reads have no owner rule on LLVM, red at `f58a7733` and at head, green on Rust: the receiver of a builtin method (`senders.values().len()`), the rest of a fresh value read for one field (`open(s).id`), a thrown-away match arm value, a store read inside a record argument of a call, a match binding put in a store wrapped in `Some`. v0.15.0 not measured.
5. `crates/ignite/tests/llvm_build.rs:417`, CONFIRMED, pre-existing, note. `release_tests_link_the_native_runtime` fails after `task build` at `f58a7733` as at head ("runtime archive without the test runner").

Held: released once in both branches of an `if`, on `break` and `continue` inside an argument, in a short circuit whose right side does not run, in a generic callee, in nested calls, with two temporaries in one call, in a thrown-away `if` branch value; a task that outlives its spawner stays usable; a spawned task that fails at once has its temporary released (LLVM only). 0 diff lines under `crates/ignite-rustgen`. terminal's four `.ll` files byte-identical between base and head. `quality/baseline.json` changes only the four `known_differences` entries and `metrics.1.1.bars.llvm.value`. Rows by `git grep` at base and head: 1.3 llvm patterns 46/46, builtins 13/13; `unsafe` lines 231/231; `#[ignore` 31/31; files over 3,000 lines 5/5 (`lower.rs` 6,073 → 6,163). Mutants M1, M2, M3 (release twice), M5, M6 caught by the unit's tests. Suite: `leak::` 6 passed, 2 failed (the adversary's two); `cargo test -p ignite-llvm` 41 passed, 1 ignored; `task test-ignite` 452 per target; clippy and fmt exit 0.

Not tested: a spawn that fails to start.

```findings
[
{"file":"crates/ignite/tests/soak/temporaries_attack.ignite","line":214,"category":"acceptance","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"a fresh end passed to a callee that rebinds it through a call and spawns with it is released by the new owner scope while the task uses it (7 is not a channel sender); it ran, leaking, at f58a7733; root cause region::fresh_value treating any call as fresh (crates/ignite-llvm/src/region.rs:119)"},
{"file":"crates/ignite-llvm/src/lower.rs","line":1603,"category":"mutant","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"the unit's tests stay green when the copy is released before its use (lower.rs:1603), without the result check (own.rs:143), without put/send temporaries (own.rs:127) and without role calls (own.rs:126); temporaries_attack.ignite now kills all four"},
{"file":"docs/slice.md","line":10147,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"30.3 drops the known-gap text, but five temporary forms are still never released on LLVM and are no longer listed"},
{"file":"crates/ignite/tests/soak/temporaries_unreleased.ignite","line":78,"category":"boundary","severity":"warning","verdict":"CONFIRMED","origin":"pre-existing","message":"a builtin receiver, the rest of a fresh value read for one field, a thrown-away arm value, a read inside a record argument, and a match binding put inside Some all leak on LLVM at f58a7733 and at head and stay flat on Rust"},
{"file":"crates/ignite/tests/llvm_build.rs","line":417,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"release_tests_link_the_native_runtime fails after task build at f58a7733 exactly as at head (runtime archive without the test runner)"}
]
```
