---
format: aep.planning-md/3
id: story:store-atomic-upsert
kind: story
status: active
title: Atomic insert-or-update on Store
summary: 'Store.upsert(id, f: fn(Option<T>) -> T): no lost write when two tasks insert the same new key'
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
- confidence: inferred
  path: docs/slice.md
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-04T04:19:08Z", actor: "human:timo", revision: 8}
- {from: "proposed", to: "active", at: "2026-11-04T04:19:08Z", actor: "human:timo", revision: 9}
---
# Story: store-atomic-upsert

## Outcome

`Store<T>` has an atomic insert-or-update, `s.upsert(id, f: fn(Option<T>) -> T) -> T`: under the
store's lock `f` receives the current value (`None` when the id is absent), its result is stored,
and the stored value is returned. Two tasks that upsert the same absent id lose neither write.

## Context

- Split out of `story:store-api-gaps` (row E2) because it is a lost-write bug in programs that
  follow the documented API, not a missing convenience.
- Observed at runtime by the playground session (2026-11-04, ESS hardening pass): 4 connections
  released by a barrier, all sending `INCR` to the same fresh key, 8 histories × 4,000 keys per
  target, checked with `ess verify conform check-history`. Rust target: 1,774 of 32,000 keys
  answered `:1` to two or more clients; LLVM target: 2,486; 8 of 8 histories not linearizable on
  both. Handwritten Rust, Go and asm servers: 0, all 8 linearizable. A planted race in a Go copy
  that passes the conformance suite was caught (333 of 2,000 keys), so the recorder can fail.
- Site: ember `ember.ignite:643-662` (increment: `db.update` answers `None`, then `db.put`).
  The same pattern at `write()` :233 and `APPEND` :418 is inferred, not tested.
- The mechanics exist: `s.update(id, f)` is atomic on both targets (`ignite_io::Store::update`,
  `ignite_rt_store_update` with the `store_update.<T>` glue), `docs/slice.md` 11.3.

## Acceptance

- `docs/slice.md` 11.3 lists `s.upsert(id, f: fn(Option<T>) -> T)`, type `T` (the stored value),
  atomic, `f` pure, effect `s.write`; the method table in section 5 likewise.
- An inline test on both targets: N tasks upsert-increment the same absent id M times each; the
  final value is N × M. The same test written with `update` then `put` is not required to fail.
- `ignite-io` and `ignite-rt` unit tests of the new method; a type-checker fixture for a wrong
  `f` type; `CODEGEN_REVISION` bumped; `task gate` green.

## Out of Scope

- E1 (non-Int keys), E3 (conditional remove), E4 (update with an outcome): stay in
  `story:store-api-gaps`.
- Changing ember: the playground session owns it.
