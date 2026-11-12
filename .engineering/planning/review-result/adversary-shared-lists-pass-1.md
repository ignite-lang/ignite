---
format: aep.planning-md/3
id: review-result:adversary-shared-lists-pass-1
kind: review-result
status: active
title: Adversary pass 1, unit shared-lists (wave 14)
relations:
- reviews: story:rust-target-shared-list-storage
revision: 1
---
red

Adversary pass on unit `shared-lists` at `3de42165` (2026-11-12): NEEDS-CHANGE; 2 red cases, 6 findings, all introduced. Tests in `crates/ignite/tests/shared_lists_adversary.rs` and `crates/ignite-io/tests/list_adversary.rs` (`2810dd82`); cases executed 202 → 206 in the `ignite` lanes and 251 → 255 in `ignite-io`.

- Introduced, fixed in `4fea3c71` and `3673bee8`: a payload of a copied value that an arm matches and only reads was bound with `List::into_vec`, which copied every element (201 allocations per read at 200 elements). The payload of a temporary is now read in place, in a native arm and in a guard arm: 0.006 allocations per read at 200 elements.
- Introduced, fixed in `4fea3c71`: `clear` of a shared list went through `make_mut`, copied every element and dropped them (203 allocations per operation at 200 elements). A shared list now takes the empty storage: 3.
- Introduced, removed in `4fea3c71`: `binds_shared` could never fire, because the checker refuses a binding inside an or-pattern (`E049`).
- Introduced, now tested: the shared-field branch of `Generator::result` and the move in `shared_value` had no test that failed without them.
- Introduced, documented in `4fea3c71`: the "Still copied" list of `docs/slice.md` now names `Option<List<T>>` and tuple fields, the inner lists of a `List<List<T>>` field at the first change, and the limit below.
- Introduced, kept and documented: dropping a chain of records that hold lists overflows the stack between 30,000 and 40,000 levels, where it was between 70,000 and 80,000; an enum chain holds 35,000 where it failed at 20,000. A fix attempt was withdrawn; the test holds 25,000 levels.

Attacked and not broken: 72 programs in four groups (value semantics under every in-place change, 34 forms that must compile on the Rust target and agree with the LLVM target, the moves and borrows of units `mysql-cost` and `call-moves`, `List<Byte>` against `Bytes`).

The coordinator reran the red cases after the correction: 6 passed; `ignite-io` 133 + 255 passed; the substrate takes no new `unsafe` site. The gate on the tree merged with `main` is recorded in the story's evidence.

```findings
[
{"file":"crates/ignite-io/src/list.rs","line":26,"category":"boundary","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"Dropping a deep chain of records that hold lists overflows the stack between 30,000 and 40,000 levels, between 70,000 and 80,000 before; kept, documented in docs/slice.md, test at 25,000 levels."},
{"file":"crates/ignite-rustgen/src/lib.rs","line":5830,"category":"acceptance","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"A payload of a copied value, matched and only read, is bound with List::into_vec and copies every element (201 allocations per read at 200 elements); fixed in 4fea3c71 and 3673bee8."},
{"file":"crates/ignite-rustgen/src/lib.rs","line":5923,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"binds_shared can never fire: the checker refuses a binding inside an or-pattern (E049); removed in 4fea3c71."},
{"file":"crates/ignite/tests/shared_lists_adversary.rs","line":228,"category":"mutant","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"The shared-field branch of Generator::result and the move in shared_value had no test that failed without them; each has one in the adversary's file."},
{"file":"docs/slice.md","line":629,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"The Still copied list left out Option<List<T>> and tuple fields, the inner lists of a List<List<T>> field at the first change, and a read-only payload of a temporary; corrected in 4fea3c71."},
{"file":"crates/ignite-io/src/list.rs","line":56,"category":"acceptance","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"clear of a shared list copied every element through make_mut and dropped them (203 allocations per operation at 200 elements); a shared list now takes the empty storage (3), fixed in 4fea3c71."}
]
```
