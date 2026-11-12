---
format: aep.planning-md/3
id: story:infer-channel-ends
kind: story
status: implemented
title: Channel ends from a Store lookup or a returned record need no annotation
summary: 'Case study G32: E042 for inferred channel-end receivers'
relations:
- informed_by: story:record-literal-receiver-type
scope:
- confidence: inferred
  path: crates/ignite-check/src/lib.rs
- confidence: inferred
  path: crates/ignite-types/src/typer.rs
- confidence: inferred
  path: crates/ignite-types/tests/fixtures
- confidence: inferred
  path: docs/slice.md
- confidence: inferred
  path: std/sync.ignite
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T15:00:48Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-11-08T21:40:02Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-11-12T12:37:07Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}, executor: "agent:claude-coordinator", correlation: "audit-2026-11-12"}
---
# Story: infer-channel-ends

## Outcome

A channel end reached through a pattern binding of a `Store` lookup or through a `let` of a call's result needs no type annotation: `ignite check` accepts both forms of G32 on both targets and checks the `tx.send` effect of the call against the inferred type.

## Context

- Case study gap G32 (`docs/research/case-study-gaps.md`), on v0.15.0 and on `main` (05574a7c8):
  - `match senders.get(id) { Some(tx) => tx.try_send(1), None => false }` with `senders: Store<Sender<Int>>`;
  - `let d = ends()` where `fn ends() -> Ends` and `type Ends { tx: Sender<Int>, rx: Receiver<Int> }`, then `d.tx.try_send(1)`.
  Both are `error[E042]: cannot check the effects of `try_send`: the type of its receiver is not declared; annotate the binding`. Workaround: `Some(tx)` becomes a `let tx: Sender<Int> = …`, `let d: Ends = ends()`.
- Where it lives: the effect check resolves a receiver from declarations alone (`docs/slice.md` 11.7): `static_type` in `crates/ignite-check/src/lib.rs` has no case for a call of a declared fn (`ExprKind::Call` resolves only `Owner.constructor(..)`), and `pattern` binds a `PatternKind::Binding` with no type taken from the scrutinee's static type (`Option<T>` of `Store.get`). The type checker reports it: `effect_site` and `unchecked_effects` in `crates/ignite-types/src/typer.rs`, for every effectful method missing from `Symbols::effect_sites`. Same class as G15 (story `record-literal-receiver-type`, a record literal).

## Acceptance

- `crates/ignite-types/tests/fixtures/`: one fixture per G32 example (a `Some(tx)` binding from `Store<Sender<Int>>.get`, a `let d = f()` whose fn returns a record holding a `Sender<Int>` and a `Receiver<Int>`), each with an empty `.expected`; plus one fixture where the effect is then missing from `requires` and the expected diagnostic is the effect error (E016), not E042, so the effect is checked rather than skipped.
- The two examples run as inline tests on both targets (`task test-ignite`): the send through the inferred end arrives at the receiver.
- `docs/slice.md` 11.7 lists the two new static-type cases; `task gate` green.

## Scope

- `crates/ignite-check/src/lib.rs`, `crates/ignite-types/src/typer.rs`, `crates/ignite-types/tests/fixtures/`, `std/sync.ignite`, `docs/slice.md`. Inferred.
