---
format: aep.planning-md/3
id: review-result:adversary-call-moves-pass-1
kind: review-result
status: active
title: Adversary pass 1, unit call-moves (wave 13)
relations:
- reviews: story:rust-target-read-only-forms-borrow
revision: 1
---
red

Adversary pass on unit `call-moves` at `bf21b392` (2026-11-12): NEEDS-CHANGE; 3 red cases, 17 findings introduced, 2 older. Tests on branch `wave/13-call-moves-adv` (`32077101`), files `crates/ignite/tests/call_moves_adversary.rs` and `crates/ignite-rustgen/tests/call_moves_adversary.rs`; cases executed 36 → 40.

- Introduced, fixed in `34d212cc`: a lent `let b = t.to_bytes()` alive across a late move of its root failed with `E0505` on the Rust target (3503, 3803, 999 on the evaluator and the LLVM target). The lent `let` is now a copy when its root is moved, late or plain, in a statement that reads the bytes or comes before one. The plain-move form failed at the base too and is fixed with it.
- Introduced, fixed in `34d212cc`: an arm that binds the whole tuple of a lent tuple `match` failed with `E0308` on the Rust target. Such an arm rebuilds the tuple as copies when it reads the name.
- Twenty guards switched off one at a time in a scratch copy: nine failed nothing in the unit's suites and are decided by the adversary's guard test (one program per guard, three consumers); six survived everything. Of those six, `34d212cc` removed four (the spawn check, the borrowed-or-captured filter on late moves, the Rust `rust_copy || lendable` item filter, the LLVM handle exclusion) and kept two with a deciding test each (`is_native`: rustc `E0614`; the late-moved parameter mark: count tests on both targets).
- Found by the implementor in the correction round and fixed in `34d212cc`, both introduced by the unit: the Rust target lent a tuple whose item held the last sender of a channel, so it was dropped at scope end (50 where the evaluator gives 51); the LLVM target lent a tuple whose arm moves a binding (4143 where the evaluator gives 4123).
- Found by the coordinator on the dedicated host: `ignite check` and `ignite plan` of the terminal design file were 2 ms and 4 ms slower in every pass at `bf21b392`. `16da8b69` and `40ac14d7` order the late-move tests and move the search out of line; the row is judged in the story's evidence.
- Older than the unit, filed as `story:late-move-in-a-loop-over-own-rows`: `for row in r.rows { r = set_in(r, row, read_in(r, row)?) }` still copies `r` for every update, because the `for` pin blocks every move of `r` in the body. Its red test is on the adversary's branch only.

Attacked and not broken: about 90 programs in nine groups (order of effects, a later argument that changes the place, early exits, later arguments that borrow, places that are not plain locals, target and call forms, forms that stay copied, tuple scrutinees, the move analysis).

The coordinator reran the unit's cases after the correction: `call_moves_adversary:: borrowed_reads:: quality::` green in the pre-gate lane; `task gate` exit 0 on `0a7ef12f` (3052 passed, 0 failed, 23 ignored; 389 UI checks).

```findings
[
{"file":"crates/ignite-types/src/moves.rs","line":1858,"category":"acceptance","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"late_moves cannot see that lent_let made b a borrow of r.label, so a lent let alive across a late move of its root fails with E0505 on the Rust target; fixed in 34d212cc."},
{"file":"crates/ignite-rustgen/src/lib.rs","line":3631,"category":"boundary","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"The same lent let across a plain move of its root fails with E0505 at the base too; fixed with the late-move form in 34d212cc."},
{"file":"crates/ignite-rustgen/src/lib.rs","line":5654,"category":"acceptance","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"lent_tuple accepts an arm that binds the whole tuple, whose binding is a tuple of references used as a tuple of values: E0308 on the Rust target; fixed in 34d212cc."},
{"file":"crates/ignite-types/src/moves.rs","line":1609,"category":"acceptance","severity":"warning","verdict":"CONFIRMED","origin":"pre-existing","message":"An update in a loop over the record's own rows still copies the record for every update: the for pin blocks every move of r in the body; filed as story late-move-in-a-loop-over-own-rows."},
{"file":"crates/ignite-types/src/moves.rs","line":1871,"category":"mutant","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"Nine guards (held_overlaps, pinned, dead-after, earlier operands, holding the late operand, take_late's own, is_ok in the evaluator's late_args, the LLVM lent_tuple changed_in and moved_in) failed nothing in the unit's suites when switched off; each is decided by one program of the adversary's guard test."},
{"file":"crates/ignite-types/src/moves.rs","line":1722,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"The spawn / non-Fn check on late moves survives every test; replaced in 34d212cc by late moves only in a fn call, decided by variant_late."},
{"file":"crates/ignite-rustgen/src/lib.rs","line":4587,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"The borrowed-or-captured filter on late moves cannot fire: the analysis never marks such a root as a late move; removed in 34d212cc."},
{"file":"crates/ignite-rustgen/src/lib.rs","line":5667,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"is_native in lent_tuple survives the adversary's programs; kept in 34d212cc with a deciding test (rustc E0614 without it)."},
{"file":"crates/ignite-rustgen/src/lib.rs","line":5679,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"The rust_copy || lendable item filter survives every test; removed in 34d212cc: a fn-typed item is lent and copied out of its reference in the arm."},
{"file":"crates/ignite-llvm/src/lower.rs","line":1850,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"The LLVM holds_handles exclusion of lent_tuple survives every test; removed in 34d212cc: a tuple literal is never an owned match."},
{"file":"crates/ignite-types/src/moves.rs","line":1734,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"Marking a late-moved parameter as moved survives every value test (a copy only); kept in 34d212cc with count tests on both targets."},
{"file":"crates/ignite-rustgen/src/lib.rs","line":5654,"category":"wrong-result","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"Found in the correction round: a lent tuple whose item holds the last sender is dropped at scope end on the Rust target (50 for 51); fixed in 34d212cc with TypedProgram::holds_handles."},
{"file":"crates/ignite-llvm/src/lower.rs","line":1856,"category":"wrong-result","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"Found in the correction round: the LLVM target lent a tuple whose arm moves a binding, and the callee wrote the place's buffer (4143 for 4123); fixed in 34d212cc."},
{"file":"crates/ignite-types/src/moves.rs","line":1858,"category":"judgement","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"Found on the dedicated host: check and plan of the terminal design file 2 ms and 4 ms slower in every pass; the late-move search is ordered (16da8b69) and out of line (40ac14d7)."}
]
```
