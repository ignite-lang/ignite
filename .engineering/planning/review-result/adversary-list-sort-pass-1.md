---
format: aep.planning-md/3
id: review-result:adversary-list-sort-pass-1
kind: review-result
status: active
title: Adversary pass 1, unit list-sort (wave 15)
relations:
- reviews: story:list-sort
revision: 1
---
red

Adversary pass on unit `list-sort` at `5219a06b` (2026-11-13): NEEDS-CHANGE; 6 findings, all introduced. Tests in `crates/ignite/tests/list_sort_adversary.rs` (`25c91d49`), eight cases, two of them red at `5219a06b`.

- Introduced, fixed in `5fce83ec`: a `sort_by` whose key fn never returns was accepted by `ignite check` (`ordered` accepts `Never`), built on the Rust target, and panicked the LLVM code generator ("no ordering for a value of type Never"). `glue::compare` has an arm for `Never` now.
- Introduced, fixed in `5fce83ec`: the Rust target's `sort_by` copied every element twice, 2,005 allocations for 1,000 texts, where `docs/slice.md` said once. The key fn reads the element where `map`'s fn does; an element that is `Copy` in Rust is passed by value.
- Introduced, fixed in `5fce83ec`: the unit's LLVM allocation test compared two single memcheck counts with tolerance 0 while one binary gives 31 or 32 `malloc` calls; it failed in 5 of 12 lane runs. It compares the largest of 6 runs.
- Introduced, now tested: dropping `sort_by` from the checker's effect guard failed none of the unit's 6 tests. The adversary's effect case fails on that mutant.
- Introduced, now tested: dropping the E045 refusal for `List<T>` or for enums with payloads failed none of the unit's 6 tests. The adversary's E045 case (16 types) fails on both mutants.
- Introduced, fixed in `5fce83ec`: the docs advised sorting an `Int` key descending by its negation, which overflows for the smallest `Int`. They give `0 - 1 - k`, pinned on the evaluator and both targets.

Attacked and not broken: the order of every listed type at its edges and of tuples nested three deep; stability at every group size and at lengths one above a power of two; the receiver unchanged in every place a list is held (local, field, element, parameter, constant, temporary, `match` binding, capture, across a `spawn`, in a `Store`); key fns of every kind, their failures and their effects; memcheck without an error on every LLVM program; the instruction ratio between 100,000 and 10,000 elements at or below 13.4 on six input forms; no sort symbol in a release binary that never sorts. Nine mutants: six killed by the unit's tests, three by the adversary's.

Not checked by the pass: whether the code `map` shares with `sort_by` generates different code for a program that only uses `map`. The golden suites of both targets are unchanged.

The coordinator reran the cases after the correction: `cargo test -p ignite --test main list_sort`, 14 passed, 0 failed.

```findings
[
{"file":"crates/ignite-llvm/src/glue.rs","line":590,"category":"boundary","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"A sort_by whose key fn has type Never is accepted by the checker (ordered includes Never) and builds on Rust, but the LLVM code generator panicked with 'no ordering for a value of type Never'. Fixed in 5fce83ec."},
{"file":"docs/slice.md","line":12039,"category":"contract-drift","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"The Rust target's sort_by copied every element twice (once into the key fn, once into the result), 2,005 extra allocations for 1,000 texts, against the documented 'copies each element once'. Fixed in 5fce83ec: the key fn is lent the element."},
{"file":"crates/ignite/tests/list_sort.rs","line":464,"category":"judgement","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"The LLVM allocation test compared two single memcheck counts with zero tolerance while the same binary gives 31 or 32 mallocs, so it failed in 5 of 12 lane runs without a defect. Fixed in 5fce83ec: the largest of 6 runs."},
{"file":"crates/ignite-check/src/lib.rs","line":2906,"category":"mutant","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"Dropping sort_by from the checker's effect guard left all 6 tests of the unit green although an inferred caller of an effectful named key fn loses its E016. The adversary's case holds it."},
{"file":"crates/ignite-types/src/lib.rs","line":326,"category":"mutant","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"Dropping the E045 refusal for List<T> (T not Byte) or for enums with payloads (line 327) left all 6 tests of the unit green. The adversary's case holds both."},
{"file":"docs/slice.md","line":12029,"category":"boundary","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"The advice to sort an Int key descending by its negation fails with an overflow on both targets for a list holding the minimum Int. Fixed in 5fce83ec: the docs give 0 - 1 - k."}
]
```
