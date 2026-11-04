---
format: aep.planning-md/3
id: story:store-api-gaps
kind: story
status: draft
title: Store keyed by text, with atomic insert and conditional remove
summary: 'Close ember''s Store gaps E1 to E4: non-Int keys, insert-or-update, conditional remove, update with an outcome'
scope:
- confidence: inferred
  path: crates/ignite-io/src
- confidence: inferred
  path: crates/ignite-llvm/src
- confidence: inferred
  path: crates/ignite-rt/src
- confidence: inferred
  path: crates/ignite-rustgen/src
- confidence: inferred
  path: crates/ignite-types/src
revision: 3
---
# Story: store-api-gaps

## Outcome

`Store<T>` serves a server keyed by text: keys other than `Int`, an atomic insert-or-update, a
conditional remove, and an update step that returns an outcome beside the new value.

## Context

- Rows E1 to E4 found by the example program ember (a Redis-protocol key-value server in Ignite),
  `examples/friction.md` in the sibling examples repository, compiler at `9578530`:
  - E1 `Store<T>` is keyed by `Int` only (`docs/slice.md` 11.3).
  - E2 no atomic insert-or-update: `update` answers `None` for an absent id and a following `put`
    can lose another task's write. Observed at runtime (2026-11-04, compiler `9578530`): 16
    clients each sent `INCR fresh:<r>` for 2,000 fresh keys after a barrier; 1 of 32,000
    increments lost on the Rust target, 2 of 32,000 on the LLVM target.
  - E3 no conditional remove.
  - E4 the fn given to `Store.update` returns only the new `T`.

## Acceptance

- Each of E1 to E4 has an inline test on both targets that fails at `9578530` and passes after;
  E2's test runs two tasks inserting the same new key and loses neither write.

## Out of Scope

- Persistence, transactions across keys.
