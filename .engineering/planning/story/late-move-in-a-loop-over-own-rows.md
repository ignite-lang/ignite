---
format: aep.planning-md/3
id: story:late-move-in-a-loop-over-own-rows
kind: story
status: draft
title: An update in a loop over the record's own rows moves the record
revision: 1
---
# Story: an update in a loop over the record's own rows moves the record

## Outcome

`for row in r.rows { r = set_in(r, row, read_in(r, row)?) }` moves `r` into the call on both targets, as the same loop over a `rows` parameter does since unit `call-moves`.

## Why

The adversary pass on unit `call-moves` (2026-11-12) found the form still copied; it was copied before the unit too. Its test is on branch `wave/13-call-moves-adv` (`32077101`): `crates/ignite-rustgen/tests/call_moves_adversary.rs`, `adversary_an_update_in_a_loop_over_the_records_own_rows_moves_the_record`. Emitted Rust:

```
for row in (r.rows.clone()) {
    r = set_in(r.clone(), row.clone(), (read_in(&r, ...))?);
```

The `for` pin (`crates/ignite-types/src/moves.rs:1609`) blocks every move of `r` in the body while the loop walks a part of `r`. Here the body assigns `r`, so both targets already iterate a copy of the rows, and the pin protects nothing: the record is copied once per round for no reader.

Whether the first user of the unit writes this form is not known; their reported loops walk a `rows` parameter.

## Acceptance

- The test is taken from the branch and is green: no `r.clone()` in the loop body on the Rust target; the LLVM target's allocation count per round for the same program is written down and does not grow with the record's size.
- A loop whose body does not assign the record keeps the pin: a program per case (the body reads `r` after the call, the body moves `r` without assigning it) gives the evaluator's value on both targets.
- `docs/slice.md` section 6 names the form; `CODEGEN_REVISION` is bumped; `task gate` green.

## Scope

- `crates/ignite-types/src/moves.rs` (the `for` pin), `crates/ignite-rustgen/tests/`, `crates/ignite/tests/`, `docs/slice.md`. Inferred.
