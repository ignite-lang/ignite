---
format: aep.planning-md/3
id: story:slice-states-the-accepted-forms
kind: story
status: draft
title: docs/slice.md states the forms the compiler accepts
revision: 1
---
# Story: docs/slice.md states the forms the compiler accepts

## Outcome

Where the compiler accepts a shorter form than `docs/slice.md` describes, the contract states the form, a test pins it, and the open row of `docs/friction.md` that the form solves is closed.

## Why

Reported on 2026-11-12 by the author of a command-line tool written in Ignite, from probes against v0.16.2: the program was written in the long forms because the contract names only those. Each case below is the reporter's probe and is unverified here; the first step of the story is a test for each.

- `for`: the grammar line says `'for' ident 'in' expr block`, and a later section says a tuple is "the only pattern besides a name and `_`" that `for` takes; reported accepted: `for (a, b) in l` and a nested `for (k, (a, b)) in m`. Row 19 of `docs/friction.md` ("`for` binds a name, not a pattern") is open.
- A `component` field: the grammar line says `ident '=' expr`; reported accepted: field punning, as the contract gives it for a record literal (`R { a, b = e }`). One example of the contract writes `command_line = command_line`.
- Imports: the contract says an import of a non-`std` module is E031 "in this slice"; reported accepted: imports of sibling modules in a program of several modules.
- Reported as hidden in the contract and working: `map_err` on a `Result`, `?` in a test body, a bare variant name where the type is known.

## Acceptance

- For each case: an inline test or a file under `crates/ignite/tests/` that holds the form on the evaluator and both targets, or, where the probe does not hold, one sentence in the report of the unit and no change of the contract.
- The grammar lines and the sections of `docs/slice.md` are corrected for every case that holds; a sentence that is true only of an earlier slice says which slice.
- Rows of `docs/friction.md` that a held case solves move to solved, with the test's name.
- `site/book` shows the short form where it shows the long one today.
- `task gate` green (tests are added).

## Scope

- `docs/slice.md`, `docs/friction.md`, `site/book`, `crates/ignite/tests/`. Inferred.

## Not in this story

- A change of what the compiler accepts.
