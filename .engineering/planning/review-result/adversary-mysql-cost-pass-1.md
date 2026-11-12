---
format: aep.planning-md/3
id: review-result:adversary-mysql-cost-pass-1
kind: review-result
status: active
title: Adversary pass 1, unit mysql-cost (wave 13)
relations:
- reviews: story:rust-target-read-only-forms-borrow
revision: 1
---
red

Adversary pass on unit `mysql-cost` at `4bcda2ab` (2026-11-12): NEEDS-CHANGE; 6 findings introduced, 3 older, 2 infeasible guards. Tests on branch `wave/13-mysql-cost-adv` (`edb6f127`), file `crates/ignite/tests/borrowed_reads_adversary.rs`.

- Introduced, fixed in `1dfc2937`: an element was lent to builtin receivers (`slice`, `pieces`, `index_of`, `at`) without a check of what a later argument does to the list; five program forms that compiled at `1e16dfba` failed with `E0506` on the Rust target. `size(l[0][j])` whose index assigns `l` no longer compiled.
- Four guards had no test (three in the Rust backend, one in the LLVM backend); one program per guard now decides each.
- Two guards could not fire and are removed; a third is load-bearing after the fix and has a test.
- `docs/slice.md` named a `for` with a variant pattern, which the parser refuses; corrected.
- Older than the unit, filed as `story:element-receiver-read-before-arguments`: the LLVM target reads the element receiver of `starts_with` and `strip_prefix` after its argument; a field of an element as a `slice` receiver does not compile on the Rust target when a bound replaces the list; the evaluator reads the index of `l[0][j]` before the receiver, both targets after.

Attacked and not broken: 78 programs in nine groups (changes of the list through nested blocks, loops, closures and record roots; bindings that escape; indexes out of range; lists that are not places; element types; comparisons; function values; tasks holding a borrowing `for` across a receive).

The coordinator reran the unit's cases after the correction: 56 passed in `borrowed_reads:: borrowed_reads_adversary:: quality::`, 4 in the Rust backend's `borrowed_reads::`; `task gate` exit 0 on the merged tree (29 suites, 389 UI checks).

```findings
[
{"file":"crates/ignite-rustgen/src/lib.rs","line":2680,"category":"acceptance","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"ref_arg lends l[i] to builtin receivers (slice, pieces, index_of, at) whose callers do not check later arguments for a change of the list, so programs that compiled before fail with E0506 (test borrowed_reads_adversary::adversary_an_element_receiver_of_a_builtin_whose_argument_changes_the_list); fixed in 1dfc2937"},
{"file":"crates/ignite-llvm/src/lower.rs","line":4707,"category":"boundary","severity":"warning","verdict":"CONFIRMED","origin":"pre-existing","message":"LLVM starts_with and strip_prefix read their element receiver after an argument that changes it, unlike Rust and the evaluator; filed as story element-receiver-read-before-arguments"},
{"file":"crates/ignite-rustgen/src/lib.rs","line":2683,"category":"boundary","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"a field of an element as a slice receiver is lent across a bound that replaces the list, E0506 at the base too; filed as story element-receiver-read-before-arguments"},
{"file":"crates/ignite-eval/src/eval.rs","line":1,"category":"boundary","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"for l[0][j] whose index replaces l the evaluator reads the index first and both targets read the receiver first; filed as story element-receiver-read-before-arguments"},
{"file":"crates/ignite-rustgen/src/lib.rs","line":4659,"category":"mutant","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"switching off reads_element_in_place's argument check left every existing test green; covered by adversary_each_guard_of_a_lent_element_decides_one_program"},
{"file":"crates/ignite-rustgen/src/lib.rs","line":2649,"category":"mutant","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"dropping lent_element from ref_args' later-argument root check left every existing test green; covered by the guard program"},
{"file":"crates/ignite-rustgen/src/lib.rs","line":4765,"category":"mutant","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"switching off compared's later-operand check for a lent element left every existing test green; covered by the guard program"},
{"file":"crates/ignite-llvm/src/lower.rs","line":1775,"category":"mutant","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"keeping match l[i] in place when an arm changes l was caught by no existing test; LLVM then gives 5 instead of 3; covered by the guard program"},
{"file":"crates/ignite-rustgen/src/lib.rs","line":2578,"category":"mutant","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"the binding-changed guard of borrowed_for_pattern is unreachable because the checker refuses changing a for binding (E006); removed in 1dfc2937"},
{"file":"crates/ignite-llvm/src/lower.rs","line":1172,"category":"mutant","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"read_root's index-changes-nothing guard changes no value, since the copy is taken after the pointer; removed in 1dfc2937. The Rust counterpart in lent_element is load-bearing after the fix and has a test"},
{"file":"docs/slice.md","line":7281,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the doc named a for with a variant pattern, which the parser refuses; corrected in 1dfc2937"}
]
```
