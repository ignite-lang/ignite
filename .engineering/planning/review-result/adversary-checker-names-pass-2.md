---
format: aep.planning-md/3
id: review-result:adversary-checker-names-pass-2
kind: review-result
status: active
title: Adversary, builtin-named-user-fn + record-literal-receiver-type, pass 2
relations:
- reviews: story:builtin-named-user-fn
- reviews: story:record-literal-receiver-type
revision: 1
---
unit: story:builtin-named-user-fn + story:record-literal-receiver-type, branch fix/checker-names at 620500a plus uncommitted test files
verdict: CONFIRMED
cases: executed 110→111, red 4 (3 fixtures, 1 test)
origin: introduced 0 / pre-existing 3 / undecided 0 (F4: the data gap pre-existing, the test that misses it from 620500a)
wrote-outside-worktree: 1 (scratch unit-checker-names/adv2)
needs-coordinator: whether F1 to F3 hold the unit or go to the backlog

## Mutation check of 620500a's table test

m1 (Ref/Signal `to_text` removed from the checker) caught; m2 (`PipeStream.received_batches` removed from the type checker) caught; m3 (`received_batches` removed from `BUILTIN_METHODS`) not caught by it (caught by the new test and by `e016_adv_bounded_effects`).

## Findings

- F1 (pre-existing) `crates/ignite-check/src/lib.rs:4179-4180`: element-type aliases defeat `builtin_applies`: `List<Octet>.to_text`, `List<Line>.join` charged to a same-named user fn (false E003). Fixture `valid_alias_element_builtin`.
- F2 (pre-existing) `lib.rs:2297`, `:4176`: `debug` on a `ByteStream`-bounded parameter is a builtin for the checker, a user fn for the type checker; the result has no roots and E016 is missed. Fixture `e016_bounded_debug_user_fn`.
- F3 (pre-existing): `x.debug(..)` on `List<T>` calling an effectful user fn gets a false E042. Fixture `valid_generic_debug_user_fn`.
- F4: `builtin_tables` iterates only the checker's names; `next_id`, `values`, `update`, `upsert` are missing from `BUILTIN_METHODS`. Test `builtin_tables::every_type_checker_builtin_name_is_known_to_the_checker`.

## Not broken

E016 on capability calls beside a same-named user fn; capability builtins in `builtin_applies` change only `remove`; E042 for unseen receivers; aliases at the head of the receiver type; a record named `Signal` (E004); pass-1 cases green. Static calls (`Int.parse`) never reach `builtin_applies`.

## Coordinator disposition

All pre-existing: backlog `story:checker-builtin-table-gaps` (draft) with the three fixtures in its body; the new test committed `#[ignore = "story:checker-builtin-table-gaps: …"]` as `b5261cb`.

```json
[
  {"id":"F1","severity":"warning","title":"Element-type alias defeats builtin_applies","evidence":"valid_alias_element_builtin.ignite:18:37 E003; lib.rs:4179","test":"fixture in story:checker-builtin-table-gaps","pre_existing":true},
  {"id":"F2","severity":"warning","title":"debug on a ByteStream-bounded param: checker builtin, type checker user fn; E016 missed","evidence":"e016_bounded_debug_user_fn expected E016, actual clean; lib.rs:2297","test":"fixture in story:checker-builtin-table-gaps","pre_existing":true},
  {"id":"F3","severity":"note","title":"x.debug(..) on List<T> calling an effectful user fn gets a false E042","evidence":"valid_generic_debug_user_fn:10:80 E042","test":"fixture in story:checker-builtin-table-gaps","pre_existing":true},
  {"id":"F4","severity":"note","title":"builtin_tables misses names absent from BUILTIN_METHODS; next_id, values, update, upsert missing","evidence":"m3 green on the_checker_table_matches_the_type_checker","test":"builtin_tables::every_type_checker_builtin_name_is_known_to_the_checker (ignored, b5261cb)","pre_existing":true}
]
```
