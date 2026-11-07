---
format: aep.planning-md/3
id: story:builtin-named-user-fn
kind: story
status: active
title: A user fn named like a builtin method is callable as a plain fn
scope:
- confidence: inferred
  path: crates/ignite-check/src/lib.rs
- confidence: inferred
  path: crates/ignite-types/src
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-07T22:06:09Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-07T22:06:09Z", actor: "human:timo", revision: 3}
---
# Story: builtin-named-user-fn

## Outcome

A user fn whose name equals a builtin method of its first parameter's type (`set` on `List<T>`) is accepted; only a method-syntax call that would resolve to the builtin is refused or warned, and a plain call `set(o, k, v)` calls the user fn on both targets.

## Context

- Found by the pusher case study (session `pusher-2b`, G14 in `docs/research/pusher-gaps.md`), ignite v0.13.0; reproducer `docs/research/pusher-gaps/G14.ignite`: `error[E041]: fn `set` takes a `List<(Text, Text)>` first, and `List<(Text, Text)>` has the builtin method `set`, so `x.set(..)` could never call it; rename it`.
- Related: `story:immutable-binding-e006` (a user fn `push` by UFCS refused with E006) and `docs/friction.md` row 54 (`to_text` reserved): the same name collision between user fns and builtins.

## Acceptance

- G14 checks clean and its plain call runs the user fn on both targets; `x.set(..)` on a `List` still calls the builtin.
- The E041 rule in `docs/slice.md` states which calls it refuses; `task gate` green.
