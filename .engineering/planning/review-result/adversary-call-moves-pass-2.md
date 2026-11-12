---
format: aep.planning-md/3
id: review-result:adversary-call-moves-pass-2
kind: review-result
status: active
title: Adversary pass 2, unit call-moves (wave 13)
relations:
- reviews: story:rust-target-read-only-forms-borrow
revision: 1
---
red

Second adversary pass on unit `call-moves`, over the correction round only (`34d212cc`, `16da8b69`), at `0a7ef12f` (2026-11-12): NEEDS-CHANGE; 1 red case, 9 findings introduced, 1 older. Tests in `crates/ignite/tests/call_moves_adversary_pass2.rs` (`980e900b` on the unit's branch); cases executed 349 → 353. The pass was taken because the correction round added code nobody had attacked.

- Introduced, fixed in `11504294`: `holds_handles` sent a fn value to "no handle", so the Rust target lent a tuple whose item held the only sender through a fn value (a record with a fn field, a list or option of fn values, a variant with a fn payload) and kept the item to the end of its scope: 50 where the evaluator gives 51. The Rust target gave 51 for all four before the unit. A fn value now counts as a possible handle holder.
- Introduced, now tested: the latch of the lent `let`'s reverse scan (rustc `E0505` without it) and six branches of `holds_handles` (list, option, generic argument, enum payload, nested record, type parameter; 50 for 51 without each) were decided by no test of the round.
- Introduced, now tested: four conditions that only save a copy (a lent `let` whose root moves after its last read, a whole-tuple binding the arm never reads, a generic record whose type argument holds no handle, a late move whose only later argument names the place) are read from the emitted Rust.
- Introduced, removed in `11504294`: the scope check on a whole-tuple binding could never be false where it ran.
- Not decidable by a test: three conditions of the late-move search change only its cost.
- Older than the unit, filed as `story:rust-target-fn-value-drops-its-sender`: a bare fn value that captured the only sender, passed to a non-consuming parameter or used as a tuple item at its last use, stays alive on the Rust target (50 for 51 at `21bc8eda` and now). No committed test.

Attacked and not broken: 11 programs in four groups over the lent `let`, the whole-tuple binding, the handle rule on every non-fn form, and the LLVM arm that moves a binding (12 forms).

The coordinator reran the red case after `11504294`: the pass-2 lane 4 passed; `task gate` on `e6e328e0` is recorded in the story's evidence.

```findings
[
{"file":"crates/ignite-types/src/lib.rs","line":1728,"category":"wrong-result","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"holds_handles_in sends Ty::Fn to no handle, so the Rust target lends a tuple whose item holds the only sender through a fn value and keeps it to scope end (50 for 51); fixed in 11504294."},
{"file":"crates/ignite-rustgen/src/lib.rs","line":3658,"category":"mutant","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"The latch of lent_let's reverse scan (read |=) was decided by no test of the round: rustc E0505 without it; covered by the pass-2 lent-let case."},
{"file":"crates/ignite-types/src/lib.rs","line":1703,"category":"mutant","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"The list, option, generic-argument, enum-payload, nested-record and type-parameter branches of holds_handles were decided by no test (50 for 51 without each); covered by the pass-2 handle-branches case."},
{"file":"crates/ignite-rustgen/src/lib.rs","line":3659,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"Four copy-saving conditions (rustgen lib.rs 3659 and 5590, types lib.rs 1726, moves.rs 1884) were decided by no test; covered by the pass-2 emitted-text case."},
{"file":"crates/ignite-rustgen/src/lib.rs","line":5588,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"The scope check on a whole-tuple binding can never be false where it runs; removed in 11504294."},
{"file":"crates/ignite-types/src/moves.rs","line":1882,"category":"mutant","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"Three conditions of the late-move search (the names prefilter, the Target arm, the loop bound) change only its cost; no value test can decide them."},
{"file":"crates/ignite-rustgen/src/lib.rs","line":5701,"category":"wrong-result","severity":"warning","verdict":"CONFIRMED","origin":"pre-existing","message":"A bare fn value that captured the only sender, passed to a non-consuming parameter or used as a tuple item at its last use, stays alive on the Rust target (50 for 51 at 21bc8eda too); filed as story rust-target-fn-value-drops-its-sender."}
]
```
