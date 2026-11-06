---
format: aep.planning-md/3
id: story:moves-for-pattern-bindings
kind: story
status: implemented
title: Moves analysis tracks pattern bindings
relations:
- derived_from: story:slice-16-rust-moves
scope:
- confidence: cited
  path: crates/ignite-llvm/src
- confidence: cited
  path: crates/ignite-rustgen/src/lib.rs
- confidence: cited
  path: crates/ignite-rustgen/tests/golden
- confidence: cited
  path: crates/ignite-types/src/moves.rs
- confidence: cited
  path: crates/ignite/src/apply/plan.rs
- confidence: cited
  path: docs/slice.md
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-06T10:00:26Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-11-06T10:00:26Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-11-06T11:11:43Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
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
