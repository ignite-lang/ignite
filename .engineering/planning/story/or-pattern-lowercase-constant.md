---
format: aep.planning-md/3
id: story:or-pattern-lowercase-constant
kind: story
status: draft
title: A lowercase constant in an or-pattern is no binding
relations:
- informed_by: story:duplicated-rules-back
revision: 1
---
# Story: a lowercase constant in an or-pattern is no binding

## Outcome

`limit | 0` in a pattern, where `limit` is a constant, checks as two alternatives and is not refused with E049.

## Why

Adversary pass 1 on unit dup of wave 11 (`review-result:adversary-dup-pass-1`, finding 1), `crates/ignite-check/src/lib.rs:4069`, pre-existing: the binding walk lists constant names, so a lowercase constant in an or-pattern is refused with E049, against `docs/slice.md` row 5 ("whatever its case"). The binary built from the unit's head minus the unit gives the same E049. Red case `or_patterns::a_lowercase_constant_in_an_or_pattern_is_no_binding`.

Found in the same wave and of the same size, taken along:

- `crates/ignite-types/src/typer.rs:666` (unit counts, pass 1, finding 5): giving a defaulted field its default explicitly (`touch = None`) keeps the record literal from waiting for its type, so `run(Box { state = "s", touch = None }, 7)` stays E042 while the same call without the field is accepted.

## Acceptance

- The red case is green; a constant and a binding of the same spelling in different scopes are told apart by a test.
- The record literal with an explicitly given default checks as the one without it.
- Both diagnostics keep their fixtures for the cases that must still be refused.
- `task gate` green; no row of `docs/quality.md` worse.

## Scope

- `crates/ignite-check/src/lib.rs`, `crates/ignite-types/src/typer.rs`, their tests. Inferred.
