---
format: aep.planning-md/3
id: story:record-literal-receiver-type
kind: story
status: implemented
title: Effects of a method call read the receiver's inferred type
relations:
- informed_by: story:immutable-binding-e006
scope:
- confidence: inferred
  path: crates/ignite-check/src/lib.rs
- confidence: inferred
  path: crates/ignite-types/src
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-07T22:06:09Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-07T22:06:09Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-11-08T02:16:34Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"test_result":1}}}
---
# Story: record-literal-receiver-type

## Outcome

The effect check of a method call reads the receiver's inferred type, so `let w = World { … }` followed by `w.numbers.put(..)` checks without an annotation.

## Context

- Found by the case-study case study (session `a case-study session`, G15 in `docs/research/case-study-gaps.md`), ignite v0.13.0, LLVM; reproducer `docs/research/case-study-gaps/G15.ignite`: `error[E042]: cannot check the effects of `put`: the type of its receiver is not declared; annotate the binding` (and the same for `get`).
- Same cause as `story:immutable-binding-e006`: `crates/ignite-check` decides from the declared (`static_type`) type only, before the type checker infers it. Both stories move the decision to where the type is known.

## Acceptance

- G15 passes `ignite test` on both targets without the annotation; E042 stays for a receiver whose type is unknown after inference.
- `task gate` green.
