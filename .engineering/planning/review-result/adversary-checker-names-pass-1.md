---
format: aep.planning-md/3
id: review-result:adversary-checker-names-pass-1
kind: review-result
status: active
title: Adversary, builtin-named-user-fn + record-literal-receiver-type, pass 1
relations:
- reviews: story:builtin-named-user-fn
- reviews: story:record-literal-receiver-type
revision: 1
---
unit: story:builtin-named-user-fn + story:record-literal-receiver-type, branch fix/checker-names at 6bcad21f plus uncommitted test files
verdict: CONFIRMED
cases: executed 31→31, red 1
origin: introduced 1 / pre-existing 1 / undecided 0
wrote-outside-worktree: 1 (scratch unit-checker-names/adv1, about 818 MB)
needs-coordinator: F1, the story Outcome ("refused or warned") against its Acceptance and slice.md 4.8 (silent builtin)

## Case

- `crates/ignite-check/tests/fixtures/valid_list_byte_to_text.{ignite,expected}` (uncommitted): a user fn `to_text(p: Point, extra: Int)` and `b.to_text()` on `b: List<Byte>`; expected clean, actual `valid_list_byte_to_text.ignite:10:41: error[E003]: wrong number of arguments to `to_text`: expected 2, found 1`. Cause: `builtin_applies` (`crates/ignite-check/src/lib.rs:4216`) lacks `to_text` on `List<Byte>`, `Ref`, `Signal`, which the type checker has (`builtins.rs:53`). Same E003 on base `f4232674`.

## Attacked, not broken

Plain calls go to the user fn and method syntax to the builtin, agreeing across the type checker, LLVM and Rust for `len`, `push`, `set`, `to_text`, `get`, `map`, `is_empty`, `copy`, `debug` on List, Text, Int, Map and records; `l[i] = v` beside a user `set`; last-use moves; bounded `S: ByteStream`; generic receivers; interpolation and assert reports; E042 for unknown receivers; record literals nested, generic, shadowed, `var`/`let`, captured; E016 and E006 correct.

## Findings

- F1 (introduced): with `fn len(t: Text) = 99` in scope, `"ab".len()` returns 2 silently on both targets; the story Outcome asked for refused or warned.
- F2 (pre-existing): `builtin_applies` omits `List<Byte>.to_text`, `Ref.to_text`, `Signal.to_text`.

## Coordinator disposition

- F1: the Outcome was corrected (revision 5, before this review's verdict reached the coordinator) to the Acceptance: method syntax keeps calling the builtin; no diagnostic; slice.md 4.8 states it.
- F2: same rule as the story (a user fn named like a builtin); sent to the implementor as round 2 with the adversary's fixture.

```json
[
  {"id":"F1","severity":"warning","title":"Method syntax silently calls the builtin while a same-named user fn for that type is in scope; the story Outcome asked for a refusal or a warning","evidence":"fn len(t: Text) = 99, \"ab\".len() == 2 with no diagnostic on LLVM and Rust; typer.rs:1027","test":"none","pre_existing":false},
  {"id":"F2","severity":"note","title":"The checker's builtin_applies lacks List<Byte>.to_text (and Ref/Signal.to_text), so any user fn to_text makes b.to_text() a call of that fn","evidence":"valid_list_byte_to_text.ignite:10:41 error[E003] expected 2, found 1","test":"crates/ignite-check/tests/fixtures/valid_list_byte_to_text.ignite + .expected","pre_existing":true}
]
```
