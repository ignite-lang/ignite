---
format: aep.planning-md/3
id: story:store-api-gaps
kind: story
status: draft
title: Store keyed by text, with conditional remove and a step that returns an outcome
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
revision: 8
---
# Story: store-api-gaps

## Outcome

`Store<T>` serves a server keyed by text: keys other than `Int` (E1), a conditional remove (E3),
and an update step that returns an outcome beside the new value (E4). E2, the atomic
insert-or-update, is done (`story:store-atomic-upsert`, PR 19).

## Context

- Rows E1 to E4 found by the example program ember (a Redis-protocol key-value server in Ignite),
  `examples/friction.md` in the sibling examples repository, compiler at `9578530`:
  - E1 `Store<T>` is keyed by `Int` only (`docs/slice.md` 11.3).
  - E2 no atomic insert-or-update: `update` answers `None` for an absent id and a following `put`
    can lose another task's write. Observed at runtime (2026-11-04, compiler `9578530`): 16
    clients each sent `INCR fresh:<r>` for 2,000 fresh keys after a barrier; 1 of 32,000
    increments lost on the Rust target, 2 of 32,000 on the LLVM target. Done by
    `story:store-atomic-upsert` (PR 19): the playground session's ESS history check at `225bda66`
    (8 histories × 4,000 keys, 4 clients per key behind a barrier) found 1,822 (Rust) and 3,079
    (LLVM) keys answered `:1` twice with ember on `update` then `put`, and 0 on both targets,
    8 of 8 histories linearizable, with ember on `upsert`.
  - E3 no conditional remove.
  - E4 the fn given to `Store.update` and `Store.upsert` returns only the new `T`. ember
    (playground session, 2026-11-04) now runs SETNX, SET NX/XX, GETSET, GETDEL, DEL, EXPIRE,
    PERSIST and INCR's overflow check each as one `upsert` step. Its 4-client race probe went from
    21 to 1,870 broken rounds of 8,000 per command to 0 on both targets. To get the command's reply
    out of the step, ember stores it in an `Outcome` field of every bucket and reads it from the
    returned value. That costs one field per bucket (24 B, inferred) and keeps the last GETSET or
    GETDEL value until the next write. Wanted: a step that returns a reply beside the new value,
    for example `upsert_with(id, f: fn(Option<T>) -> Step<T, R>) -> R`.

## Acceptance

- Each of E1, E3 and E4 has an inline test on both targets that fails at `9578530` and passes
  after; E2 keeps the `upsert` tests of PR 19.

## Out of Scope

- Persistence, transactions across keys.
