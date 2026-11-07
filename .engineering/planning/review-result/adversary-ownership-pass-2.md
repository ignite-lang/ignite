---
format: aep.planning-md/3
id: review-result:adversary-ownership-pass-2
kind: review-result
status: active
title: Adversary, shared-ownership-analysis, pass 2
relations:
- reviews: story:shared-ownership-analysis
revision: 1
---
unit: story:shared-ownership-analysis, worktree `ownership` at b0aa06b7 plus my uncommitted test file
verdict: NEEDS-CHANGE
cases: executed 287→290, red 3
origin: introduced 0 / pre-existing 2 / undecided 0
wrote-outside-worktree: 23 paths, all under the assigned scratch directory adversary2/
needs-coordinator: decide whether the immutable-binding half of E006 (findings 1 and 2) is in this unit's scope. The correction moved only the closure half to the type checker.

## 1. Diff stat

```
 crates/ignite-types/tests/main.rs | 1 +
 1 file changed, 1 insertion(+)
?? crates/ignite-types/tests/ownership_adversary2.rs
```
Both paths are test files. No implementation file was touched and nothing was committed.

## 2. Cases

All three are in `crates/ignite-types/tests/ownership_adversary2.rs` and are red now. They check and then type the program, the same way as `ownership::diagnostics`. This is the run of the three alone, before any suite run:

```
$ CARGO_INCREMENTAL=0 cargo test -p ignite-types --test main -- ownership_adversary2::
test ownership_adversary2::a_ufcs_call_named_push_on_a_let_is_no_change ... FAILED
test ownership_adversary2::a_map_change_through_a_let_of_undeclared_type_is_e006 ... FAILED
test ownership_adversary2::a_map_change_through_a_pattern_or_for_binding_of_undeclared_type_is_e006 ... FAILED
---- a_ufcs_call_named_push_on_a_let_is_no_change ----
panicked at crates/ignite-types/tests/ownership_adversary2.rs:136:5:
assertion `left == right` failed: a pure UFCS call was refused as an in-place change
  left: "check: probe.ignite:9:10: error[E006]: cannot `push` through the immutable binding `h`: only a place rooted in a `var` or a parameter changes in place; copy it into a `var` first\n"
 right: ""
---- a_map_change_through_a_let_of_undeclared_type_is_e006 ----
panicked at crates/ignite-types/tests/ownership_adversary2.rs:69:9:
a Map change through an immutable binding at probe.ignite:11:3 passed; the Rust target fails E0596 where LLVM runs it:
(empty: both phases passed)
---- a_map_change_through_a_pattern_or_for_binding_of_undeclared_type_is_e006 ----
panicked at crates/ignite-types/tests/ownership_adversary2.rs:108:9:
a Map change through a binding at probe.ignite:8:7 passed; the Rust target fails E0596 where LLVM runs it:
(empty)
test result: FAILED. 0 passed; 3 failed; 0 ignored; 78 filtered out
exit 101
```

- **`a_map_change_through_a_let_of_undeclared_type_is_e006`** expects E006 at three places, none of which declares the binding's type:
  - `let m = make(); m.put(..)`
  - `let m = same(make()); m.put(..)`
  - `let b = make_box(); b.m.remove(..)`
- **`a_map_change_through_a_pattern_or_for_binding_of_undeclared_type_is_e006`** expects E006 for `Some(m) => m.put(..)` and for `for m in ms { m.put(..) }`.
- **`a_ufcs_call_named_push_on_a_let_is_no_change`** expects a pure user fn `push(h: Holder, x)`, called by UFCS on `let h = same(Holder{..})`, to check clean.

Backend evidence, from probes run with the worktree's `target/debug/ignite`:
- **`probes/q8.ignite`** (`let m = make(); m.put("a", 1)`):
  - `--target llvm`: `1 passed`.
  - `--target rust`: `error[E0596]: cannot borrow m as mutable` at `{ m.insert(String::from("a"), 1); }`.
- **`q6`** (the generic-result form): same split between the targets.
- **`q10`** (the field, match and for forms): LLVM `3 passed`; Rust fails with 3 × E0596 (`let mut b`, `Some(mut m)`, `for mut m`).
- **`q7`**: with `let m: Map<Text, Int>` declared, the same call is E006 ("cannot `put` through the immutable binding `m`").

## 3. Suite run (after the cases existed)

```
$ CARGO_INCREMENTAL=0 cargo test --no-fail-fast -p ignite-types -p ignite-check -p ignite-llvm -p ignite-rustgen
ignite_check unittests  test result: ok. 10 passed; 0 failed; 0 ignored
ignite-check tests/main test result: ok. 21 passed; 0 failed; 1 ignored
tests/main              test result: ok. 37 passed; 0 failed; 1 ignored
tests/main              test result: ok. 141 passed; 0 failed; 1 ignored
ignite-types tests/main test result: FAILED. 78 passed; 3 failed; 0 ignored
doc-tests               4 × 0 passed
exit 101
```
- Totals: 287 passed, 3 failed (my three), 3 ignored.
- The "before" count of 287 is this same run with my file `ownership_adversary2.rs` excluded.
- Pass 1's two non-ignored cases are green: `ownership_adversary::closure_changing_a_captured_map_of_undeclared_type_is_e006 ... ok` and `ownership_adversary::ufcs_push_on_a_field_of_a_capture_is_no_e006 ... ok`.

## 4. Findings (tree b0aa06b7)

**1. A `Map` change through an immutable binding of undeclared type gets no E006, and the targets disagree.**
- Where: `crates/ignite-check/src/lib.rs:2745`.
- Verdict NEEDS-CHANGE, origin pre-existing.
- Cause:
  - The immutable-binding rule (`mutating`, which leads to `mutable_receiver` at :2768) still reads only `static_type`.
  - With no type, `in_place(None, "put" | "remove")` is `None`.
  - The type checker has no let/pattern/for rule.
  - The new `Local::result` is deliberately read by the closure rule only, so the rule misses even `let m = make()`, where `make` is non-generic and the checker holds its return type.
- What was measured: 2 red cases. LLVM runs these programs; Rust fails with E0596.
- What reaches it: any `let x = f()`, a match binding or a `for` element of `Map` type, then `.put` or `.remove`. No unusual setup is needed.
- Base: `6fa8da18`, built in scratch, passes `check` on q6, q8 and q10, and the Rust build of q8 fails the same way. This is pass 1's finding 1, which was fixed for captures only, here on the other half of the same E006 row.
- Fix: in `Typer::captured_change`'s place, also report E006 when a Value change is rooted at a let, pattern or for binding. Or feed `in_place_type` into `mutating`.

**2. A user fn named `push` on a `let` of undeclared type is refused.**
- Where: `crates/ignite-check/src/lib.rs:2745` (`in_place(None, "push") == Value && builtin_method(..)`).
- Verdict NEEDS-CHANGE, origin pre-existing (the base gives the same E006 on q9).
- What was measured: 1 red case.
- What reaches it: a UFCS user fn named `push`, `set`, `append` or `clear` on a `let` whose type comes from a generic call.
- The correction stopped this refusal inside closures (`ownership::a_ufcs_call_named_push_on_a_capture_is_no_change`), but the same call outside a closure is still refused. So it is the same false positive, moved to the other rule.

## 5. Attacked and could not break

- **Duplicate E006 from the two sources:** not possible. `type_parsed_with` and `check_siblings` return after any checker diagnostic before typing. A captured, declared `let` with `push` (q11) gives exactly one E006.
- **Spans and messages:** both sources use `expr.span` and the same text. `check --json` and `check --stdin-for --json` give identical output on the type-checker-only E006 (q3b).
- **A capture through a generic fn's result, a nested closure, or a generic receiver `List<T>`:** E006 in all three (q3b). For an outer closure's parameter captured by an inner closure, the checker reports E006 (q3).
- **Spawn bodies:** `spawn` accepts only a call to an effectful fn (E013 for anything else), so a spawn body cannot hold a capture.
- **The targets on programs the correction now admits:** both agree.
  - Consuming UFCS `push` on a field of a capture, on a generic capture and on a declared capture (q1): 3/3 passed on each target.
  - A capture passed to a fn that changes its copy (q2): 2/2 passed on each target.
  - `xs.at(0).push` and `(if .. {m} else {m}).put` on captures (q4): 2/2 passed on each target.
- **The exhaustive match:** `table()`, and with it the new assert, is called only by tests (no caller in any `src/`). So a wrong IN_PLACE row panics at test time, never in the compiler for users. A mutant on a HEAD copy (`PipeStreamReceivedBatches => Some(Change::Value)`) is caught, but only incidentally: by `golden::bounded_streams` (ignite-rustgen); `mutates_is_the_table` checks just 7 named negatives.

## 6. Paths written outside the worktree

All 23 are under `/home/timo/.cache/scratch/e18a99fc-f99c-4987-93c9-e5de2080b61a/unit-ownership/adversary2/`, 1.3 GB in total. The 2.3 GB `mutant-target/` was deleted.
- Logs: `build.log`, `base-build.log`, `cases.log`, `suite.log`, `mutant.log`.
- `probes/` (q1 to q11 `.ignite`, q3b).
- `base/` (git archive 6fa8da18).
- `base-target/` (698 MB).
- `mutant/` (git archive HEAD, mutated `ownership.rs`).
- `mutant-target/` (deleted).
- Program output directories: `out-q1-llvm`, `out-q1-rust`, `out-q2-llvm`, `out-q2-rust`, `out-q4-llvm`, `out-q4-rust`, `out-q6-llvm`, `out-q6-rust`, `out-q8-llvm`, `out-q8-rust`, `out-q10-llvm`, `out-q10-rust`, `out-base-q8-llvm`, `out-base-q8-rust`.

## 7. Findings block

```findings
[
  {"file": "crates/ignite-check/src/lib.rs", "line": 2745, "category": "acceptance", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "pre-existing", "message": "Map.put/remove through a let, pattern or for binding whose type the checker cannot tell gets no E006 from either phase, so LLVM runs the program and the Rust target fails E0596; the correction moved only the closure half of E006 to the type checker."},
  {"file": "crates/ignite-check/src/lib.rs", "line": 2745, "category": "boundary", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "pre-existing", "message": "A pure UFCS user fn named push on a let of undeclared type is refused as E006 through the immutable binding, the same false positive the correction removed for closures only."}
]
```
