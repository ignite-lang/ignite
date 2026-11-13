---
format: aep.planning-md/3
id: story:indexed-assignment-places
kind: story
status: draft
title: An assignment place takes an index step
relations:
- informed_by: story:parameters-changed-in-place
revision: 1
---
# Story: an assignment place takes an index step

## Outcome

`xs[i].field = v`, `xs[i] = v` inside a longer path and `m.rows[i].count = n` are assignments, so a field of a list element is changed in one line where today it is copied out, changed and written back.

## Why

Row 41 of `docs/friction.md` is open since 2026-11-02: "An assignment place is a name followed by field steps only: `buckets[i].good = n` does not parse"; the grammar of `docs/slice.md` is `place := ident ('.' ident)*`. Reported again on 2026-11-12 by the author of a command-line tool written in Ignite, as the three-line form `var top = stack[i]`, a change of `top`, `stack[i] = top`.

## Acceptance

- `place := ident (('.' ident) | ('[' expr ']'))*`: field and index steps in any order; the index is evaluated once, before the right side or after it by a stated rule that a test pins.
- An index out of range fails as `xs[i] = v` fails today; a `Map` step is in this story or named as not covered, with the reason.
- In-place methods accept the same places: `xs[i].items.push(x)` changes the element (a row of `ignite_check::IN_PLACE` per method, as for a field path today).
- Value semantics hold: a copy of the list taken before the assignment is unchanged, on the evaluator and both targets; a differential test holds one program on all three.
- Allocations on the Rust target: `xs[i].n = v` on a list of records 0; `xs[i].items.push(x)` does not copy `xs[i]` (counted at two sizes); the LLVM target's counts are written down.
- The effect roots of the place are those of its first name, as for a field path.
- Row 41 of `docs/friction.md` moves to solved; the workaround in `std/control.ignite` that the row names is rewritten; `docs/slice.md` grammar and section; `task gate` green.

## Scope

- The parser and checker crates, `crates/ignite-types/`, `crates/ignite-rustgen/src/lib.rs`, `crates/ignite-llvm/`, the evaluator, `std/control.ignite`, `docs/slice.md`, `docs/friction.md`. Inferred.

## Not in this story

- Assignment to a tuple pattern (`story:parameters-changed-in-place` decides it).
