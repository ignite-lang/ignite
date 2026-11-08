---
format: aep.planning-md/3
id: story:unowned-handle-ratchet
kind: story
status: proposed
title: A new unowned runtime handle fails the gate before any program runs
summary: own.rs's give-ups listed by ignite emit --unowned and held to a shrinking baseline over std and examples
relations:
- decomposes: epic:leak-guard
- depends_on: story:channel-release-on-close
scope:
- confidence: inferred
  path: crates/ignite-llvm/src/lib.rs
- confidence: inferred
  path: crates/ignite-llvm/src/own.rs
- confidence: inferred
  path: crates/ignite-llvm/tests
- confidence: inferred
  path: crates/ignite/src/lib.rs
- confidence: inferred
  path: crates/ignite/tests/fixtures
- confidence: inferred
  path: docs/slice.md
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T15:15:43Z", actor: "human:timo", revision: 3}
---
# Story: unowned-handle-ratchet

## Outcome

Every place the LLVM ownership lowering leaves a runtime handle unowned (never released) is listed at compile time, and the gate fails when a change to the compiler, `std/` or `examples/` adds one, before any program runs.

## Context

- Design: `docs/leak-guard.md` 1.2 (the leak class: an owned handle moved into a place the ownership lowering does not track), 4 (option d weighed against `own.rs`), Q8, Q12.
- `crates/ignite-llvm/src/own.rs` gives up at any mention other than a method receiver, a `spawn` argument or a plain call argument; G30 was four such give-ups on channel ends (`story:channel-release-on-close`). A diagnostic is wrong (the forms are correct programs); a list held to a baseline is the static half of the guard; the census (`story:leak-census`) is the runtime half.
- After `story:channel-release-on-close`, which changes the rule; the baseline is taken from its result.

## Acceptance

- `ignite emit --unowned FILE` prints one line per give-up: file, line, binding, handle type and the mention that made it unowned; a fixture with the four G30 forms applied to a `PipeStream` (not fixed by the channel story) lists four lines, and the same fixture with plain bindings lists none.
- A test in `crates/ignite-llvm/tests/` runs the listing over every file of `std/` and `examples/` and compares it with `crates/ignite-llvm/tests/unowned.baseline`; a new line fails with the line printed; a removed line fails until the baseline is rewritten (`IGNITE_BLESS=1`), so the list only shrinks deliberately.
- `docs/slice.md` states the listing; `task gate` green.

## Scope

- `crates/ignite-llvm/src/own.rs` (record the give-ups), `crates/ignite-llvm/src/lib.rs`, `crates/ignite/src/lib.rs` (`emit --unowned`), `crates/ignite-llvm/tests/` (the test and `unowned.baseline`), `crates/ignite/tests/fixtures/`, `docs/slice.md`. Inferred.
