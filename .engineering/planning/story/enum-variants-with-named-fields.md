---
format: aep.planning-md/3
id: story:enum-variants-with-named-fields
kind: story
status: draft
title: An enum variant may name its fields
revision: 1
---
# Story: an enum variant may name its fields

## Outcome

A variant is declared as `V { a: T, b: U }`, built as `V { a = x, b = y }` and matched as `V { a, b }` or `V { a = pattern, .. }`, on the evaluator and both targets.

## Why

Requested on 2026-11-12 by the author of a command-line tool written in Ignite: a variant takes positional payloads only (the grammar of `docs/slice.md`: `variant := ident ('(' type (',' type)* ')')?`), so a variant with several payloads of one type is read by position at every match.

## Acceptance

- A variant with named fields checks, builds and matches as the outcome states; a missing or unknown field is the diagnostic a record gives for the same mistake.
- A pattern may name a part of the fields with `..`; without `..` every field is named (exhaustiveness as for a record pattern).
- Positional and named variants may stand in one enum; one variant is one or the other.
- The value prints and compares as a positional variant with the same payloads does.
- One differential program covers building, matching and equality on the evaluator and both targets; the layout on the LLVM target is the positional variant's (a test compares the two sizes).
- `ignite fmt` keeps the form; `docs/slice.md` and `site/book` state it; `task gate` green.

## Scope

- `crates/ignite-syntax/src/parser.rs`, `crates/ignite-types/src/typer.rs` (variants, patterns, exhaustiveness), `crates/ignite-rustgen/src/lib.rs`, `crates/ignite-llvm/`, the evaluator, the formatter, `docs/slice.md`. Inferred.

## Not in this story

- Field access on an enum value without a match.
