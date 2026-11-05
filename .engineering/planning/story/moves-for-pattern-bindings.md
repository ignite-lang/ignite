---
format: aep.planning-md/3
id: story:moves-for-pattern-bindings
kind: story
status: draft
title: Moves analysis tracks pattern bindings
relations:
- derived_from: story:slice-16-rust-moves
revision: 1
---
# Story: moves-for-pattern-bindings

## Outcome

The moves analysis tracks owned pattern bindings, `for` elements and closure parameters, so the
Rust target moves them at their last use instead of copying them at an `if`/`match` tail.

## Context

- Found by the U3 adversary fix (2026-11-05, `93881c7e` on `feat/slice16-language-gaps`):
  `Generator::result` now copies every untracked aggregate place at a branch tail, because
  `TypedProgram::moves` does not mark pattern bindings, `for` elements or closure parameters.
- Cost measured in goldens: `Ok(stream) => stream.clone()` (`exits.rs`, `modules.rs`,
  `services.rs`, `syntax.rs`), `Some(present) => present.clone()` (`generics.rs`); a binding of a
  borrowed scrutinee is copied twice (`let present = (present).clone(); present.clone()`).
  `std.option.unwrap_or` has the same shape (inferred, its generated code not checked).
- Needs a change to `docs/slice.md` 20.3.1 (the places the analysis tracks); D1 forbids a
  rustgen-only rule.

## Acceptance

- The five golden hunks above lose their `.clone()`; the `moves_tails*` fixtures stay green on
  both targets.

## Out of Scope

- New syntax.
