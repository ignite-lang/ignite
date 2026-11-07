---
format: aep.planning-md/3
id: review-result:adversary-ownership-pass-1
kind: review-result
status: active
title: Adversary, shared-ownership-analysis, pass 1
relations:
- reviews: story:shared-ownership-analysis
revision: 1
---
unit: story:shared-ownership-analysis, worktree ownership at fa52517b (code 766932cd) plus my test file, uncommitted
verdict: NEEDS-CHANGE
cases: executed 283→286, red 3
origin: introduced 3 / pre-existing 2 / undecided 0
wrote-outside-worktree: 11 paths, all under the assigned scratch directory
needs-coordinator: decide whether finding 1 (the closure fix in the commit message only works when the type is declared) holds the unit, or goes to the backlog as pre-existing

## 1. Diff stat

```
 crates/ignite-check/tests/main.rs | 1 +
 1 file changed, 1 insertion(+)
?? crates/ignite-check/tests/ownership_adversary.rs
```
Both paths are test files. No implementation file was touched and nothing was committed.

## 2. Cases

All three are in `crates/ignite-check/tests/ownership_adversary.rs`. Each is red now. This is the run of the three alone, before any suite run:

```
$ CARGO_INCREMENTAL=0 cargo test -p ignite-check --test main -- ownership_adversary::
test ownership_adversary::closure_changing_a_captured_map_of_undeclared_type_is_e006 ... FAILED
test ownership_adversary::map_put_of_a_handle_keeps_its_roots ... FAILED
test ownership_adversary::ufcs_push_on_a_field_of_a_capture_is_no_e006 ... FAILED
---- closure_changing_a_captured_map_of_undeclared_type_is_e006 ----
panicked at crates/ignite-check/tests/ownership_adversary.rs:61:5:
m.put on a captured Map of undeclared type passed the checker:
---- map_put_of_a_handle_keeps_its_roots ----
panicked at crates/ignite-check/tests/ownership_adversary.rs:125:5:
a TcpStream put into a Map lost its roots, no E016:
probe.ignite:4:10: error[E016]: missing effect `s.read` for `read`; add it to the `requires` of `via_list`
---- ufcs_push_on_a_field_of_a_capture_is_no_e006 ----
panicked at crates/ignite-check/tests/ownership_adversary.rs:94:5:
assertion `left == right` failed: a pure UFCS call was refused as an in-place change
  left: "probe.ignite:10:20: error[E006]: cannot change `w` inside a closure; a closure captures `w` by copy\n"
 right: ""
test result: FAILED. 0 passed; 3 failed; ... 19 filtered out
exit 101
```

- **`closure_changing_a_captured_map_of_undeclared_type_is_e006`** asserts E006 for `m.put` on a captured `var m = make()`, and for `b.m.remove` on a captured `var b = make_box()`.
  - Both targets compiled the same program (`probes/p2.ignite`). LLVM: `2 passed`. Rust: `error[E0525]: expected a closure that implements the Fn trait, but this closure only implements FnMut` at `{ m.insert(String::from("a"), x.clone()); }` and at `b.m.remove(...)`.
  - That is the disagreement the commit calls fixed ("Disagreement 1"), still there whenever the capture's type is not declared.
- **`ufcs_push_on_a_field_of_a_capture_is_no_e006`** asserts that a pure user fn `push(h: Holder, x: Int) -> Int`, called by UFCS as `w.h.push(x)` inside a closure, checks clean.
  - The same call outside a closure checks clean and passes on both targets (`probes/p5.ignite`: `1 passed` LLVM, `1 passed` Rust).
- **`map_put_of_a_handle_keeps_its_roots`** asserts E016 for `via_map`, which puts a `TcpStream` into a `Map`, reads it back with `get` and calls `read` without `requires [s.read]`. The `List.push` version of the same program gets E016.

**Base run.** I ran the same three cases against a `git archive 6fa8da18` copy in scratch, with its own target dir:
- `map_put_...` and `closure_..._undeclared_type_...` fail there too, so both are pre-existing.
- `ufcs_push_...` passes on the base, so that defect is introduced.

## 3. Suite run (after the cases existed)

```
$ CARGO_INCREMENTAL=0 cargo test --no-fail-fast -p ignite-types -p ignite-check -p ignite-llvm -p ignite-rustgen
ignite_check unittests  test result: ok. 10 passed; 0 failed
ignite-check tests/main test result: FAILED. 19 passed; 3 failed; 0 ignored
tests/main              test result: ok. 37 passed; 0 failed; 1 ignored
tests/main              test result: ok. 141 passed; 0 failed; 1 ignored
tests/main              test result: ok. 76 passed; 0 failed; 0 ignored
doc-tests               4 × 0 passed
error: 1 target failed
exit 101
```
Totals: 283 passed, 3 failed (my three), 2 ignored. The "before" count of 283 is this same run with my three cases excluded.

## 4. Findings

**1. E006 is missed for a `Map` change on a capture whose type is not declared.**
- Where: `crates/ignite-check/src/lib.rs:2691`, and `in_place` at `:474`.
- Verdict NEEDS-CHANGE, origin pre-existing, tree fa52517b.
- What was measured: case 1 is red. Rust fails with E0525 while LLVM changes the closure's copy.
- Cause: when `static_type` is unknown, `in_place(None, "put" | "remove")` returns `None`, because the `Map` rows (Value) and the `Store` rows (Handle) disagree.
- What reaches it: any `var m = some_fn()` captured by a closure, which needs no unusual setup.
- The commit message and the E006 row in `docs/slice.md` section 8 both claim this case is now refused.
- Fix: decide E006 where the callee is known, in `ignite-types` (`TypedProgram::callee`), or refuse `put`/`remove` on an untyped capture.

**2. E006 now refuses a valid program.**
- Where: `crates/ignite-check/src/lib.rs:2693-2695`.
- Verdict NEEDS-CHANGE, origin introduced.
- What was measured: case 2 is red.
- Cause: the new `place_root` arm turns `builtin_method`'s "unknown type, so assume a builtin" into a refusal for a field of a capture, so a user fn named like a builtin is refused.
- What reaches it: a UFCS call of a user fn named `push`, `set`, `append` or `clear` on a field of a capture whose type comes from a call.
- The bare-name form (`h.push(x)`) was already refused on the base; reading the base checker, it refused only bare-name receivers.

**3. `Map.put` of a handle loses its roots.**
- Where: `crates/ignite-check/src/lib.rs:2709`.
- Verdict CONFIRMED, origin pre-existing.
- What was measured: case 3 is red. A `TcpStream` stored in a `Map` is read with no effect required.
- Cause: the checker extends a receiver's roots only for `push`, `set` and `append`, a set kept apart from `IN_PLACE` on purpose.
- What reaches it: any program that stores a stream in a `Map`. This is an effect-soundness gap.

**4. The LLVM debug check can no longer fail.**
- Where: `crates/ignite-llvm/src/lib.rs:117`.
- Verdict CONFIRMED (judgement), origin introduced.
- The `debug_assert!` that the modified parameters are a subset of the consumed ones now compares `changed_in_block` against `consumed`. `consumed` is built from the same `changed_in_block` (`moves.rs:57`), so the assertion is always true. It was the only cross-check between two independently computed sets.

**5. A new builtin no longer forces a decision.**
- Where: `crates/ignite-types/src/ownership.rs:62`.
- Verdict CONFIRMED (judgement), origin introduced.
- `region::mutates` was an exhaustive match: a new `Builtin` had to be classified or the build failed. `ownership::change` is a table lookup, so a new receiver-changing builtin silently counts as read-only on every target and in the checker.

## 5. Attacked and could not break

None of these turned up a difference between the targets.
- Both targets agree on:
  - fields of fields and tuple fields;
  - UFCS user fns named `push`/`clear` on a name and on an indexed place;
  - a later argument pushing to an earlier argument;
  - `for` over a list the body pushes to, directly and through `at`;
  - nested closures.
- `Store` mutators work through a field, an index, a `for` element, a closure and a nested closure (`probes/p1`, `p6`, `p7`). `Store` methods in `ignite-io` take `&self`, so the new Rust `mut` is cosmetic.
- Mutant `calls_in_place → false` on a HEAD copy is caught by `golden::executable_components`, `golden::generics_stores_and_builtins` and `moves::consumed_is_a_fixpoint`. Any edit to the table itself is caught by the `the_one_table` pin.
- Both golden changes (`components.rs`, `generics.rs`) are named in the commit, and no LLVM golden changed.
- I did not attack spawn bodies or generic receivers beyond `Store<T>` and `List<T>`.

## 6. Paths written outside the worktree

All are under `/home/timo/.cache/scratch/e18a99fc-f99c-4987-93c9-e5de2080b61a/unit-ownership/adversary/`, 235 MB in total. The 2.3 GB mutant target dir was deleted.
- `probes/` (p1 to p7 `.ignite`)
- `out/`
- `rtarget/`
- `base/`
- `base-target/`
- `mutant/`
- `cases.log`
- `base-cases.log`
- `suite.log`
- `mutant.log`
- `mutant-target/` (deleted)

## 7. Findings block

```findings
[
  {"file": "crates/ignite-check/src/lib.rs", "line": 2691, "category": "acceptance", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "pre-existing", "message": "Map.put/remove on a closure capture whose type is not declared gets no E006 because in_place(None, ..) is None, so the Rust target fails E0525 while LLVM changes a copy, contrary to the commit message and the E006 row in docs/slice.md."},
  {"file": "crates/ignite-check/src/lib.rs", "line": 2693, "category": "boundary", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "The new place_root arm refuses with E006 a pure UFCS user fn named push called on a field of a capture of undeclared type, a program that checks and runs outside a closure."},
  {"file": "crates/ignite-check/src/lib.rs", "line": 2709, "category": "judgement", "severity": "warning", "verdict": "CONFIRMED", "origin": "pre-existing", "message": "Root extension covers push/set/append only, so a TcpStream put into a Map and read back via get needs no effect (missing E016)."},
  {"file": "crates/ignite-llvm/src/lib.rs", "line": 117, "category": "judgement", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "The debug_assert that modified params are consumed now compares changed_in_block against a set derived from changed_in_block and cannot fail."},
  {"file": "crates/ignite-types/src/ownership.rs", "line": 62, "category": "judgement", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "Replacing region::mutates' exhaustive match with a table lookup means a new receiver-changing Builtin defaults to read-only everywhere without a compile error."}
]
```
