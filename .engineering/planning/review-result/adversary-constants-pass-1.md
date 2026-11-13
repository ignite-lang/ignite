---
format: aep.planning-md/3
id: review-result:adversary-constants-pass-1
kind: review-result
status: active
title: Adversary pass 1, unit constants (wave 15)
relations:
- reviews: story:rust-target-constants-built-once
revision: 1
---
red

Adversary pass on unit `constants` at `a79c9b96` (2026-11-12): NEEDS-CHANGE; 3 findings, all introduced. Tests in `crates/ignite/tests/constants_adversary.rs` (`d990249d`), three cases.

- Introduced, fixed in `dee6c9b1`: a constant of fn type lent to a parameter that its callee only calls (`apply(F, 3)`) no longer compiled on the Rust target. The lent argument was `&*F()`, a `&Arc<dyn Fn>`, which rustc refuses (E0277); it compiled at `1febe156`. It is now `&**F()`. Every other position of a fn constant has an inline test on the evaluator and both targets.
- Introduced, fixed in `dee6c9b1`: four reads that `docs/slice.md` section 6 called free still allocated on the Rust target: a constant as a `Map` key 1 per round, as the argument of `starts_with` 1, as the iterable of a `for` over a `Map` 4, and a field of an element of a list constant 1. The first three are 0 now. The fourth stays 1, the copy a local list has too; the docs name it with `story:rust-target-read-only-bindings-borrow`.
- Introduced, now tested: inverting the `Copy` rule for an `Option` constant failed no test. The `consts` golden holds `Option<Text>`, `Option<record>` and `Option<Int>` constants since `dee6c9b1`.

Attacked and not broken: every change in place of a copy of a constant, and of the constant itself, leaves the constant's value on the evaluator and both targets (lists, an inner record, a holder record, a map, an enum payload, across a `spawn`).

The coordinator reran the cases after the correction: `constants_adversary` 3 passed, `constants` 5 passed. The gate on the tree merged with `main` is recorded in the story's evidence.

```findings
[
{"file":"crates/ignite-rustgen/src/lib.rs","line":2722,"category":"acceptance","severity":"blocker","verdict":"NEEDS-CHANGE","origin":"introduced","message":"A constant of fn type lent to a parameter that only calls it was emitted as &*F(), a &Arc<dyn Fn>, which rustc refuses (E0277); it compiled before the unit. Fixed in dee6c9b1 (&**F())."},
{"file":"docs/slice.md","line":305,"category":"contract-drift","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"Four reads the docs called free allocated on the Rust target: a constant as a Map key 1 per round, as the argument of starts_with 1, as the iterable of a for over a Map 4, a field of an element of a list constant 1. The first three are 0 since dee6c9b1; the fourth stays 1 and is named in the docs with story rust-target-read-only-bindings-borrow."},
{"file":"crates/ignite-rustgen/tests/golden/consts.ignite","line":1,"category":"mutant","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"Inverting the Copy rule for an Option constant failed no test; the consts golden holds Option<Text>, Option<record> and Option<Int> constants since dee6c9b1."}
]
```
