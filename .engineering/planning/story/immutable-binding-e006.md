---
format: aep.planning-md/3
id: story:immutable-binding-e006
kind: story
status: draft
title: E006 through immutable bindings decided where the callee is known
relations:
- informed_by: review-result:adversary-ownership-pass-2
revision: 1
---
# Story: immutable-binding-e006

## Outcome

E006 for an in-place change through an immutable binding is decided where the callee is known, so both targets agree: a `Map.put`/`remove` through a `let`, match or `for` binding of any type is refused, and a user fn named like a builtin is not.

## Context

- Found by the second adversary of `story:shared-ownership-analysis` (`review-result:adversary-ownership-pass-2`, 2026-11-07); pre-existing on base `6fa8da18`. That story moved the closure half of E006 to the type checker; this is the immutable-binding half.
- Site: `crates/ignite-check/src/lib.rs:2745` (`mutating`, `mutable_receiver` ~:2768) reads only `static_type`; with no type, `in_place(None, "put" | "remove")` is `None`, and `in_place(None, "push")` is assumed a builtin.
- Red cases, `#[ignore = "story:immutable-binding-e006"]` in `crates/ignite-types/tests/ownership_adversary2.rs` (commit `eaf8e971`, on main after wave 4):
  - `a_map_change_through_a_let_of_undeclared_type_is_e006` (`let m = make(); m.put(..)`, a generic call's result, a field of a let): LLVM runs it, Rust fails E0596.
  - `a_map_change_through_a_pattern_or_for_binding_of_undeclared_type_is_e006` (`Some(m) => m.put(..)`, `for m in ms { m.put(..) }`): same split.
  - `a_ufcs_call_named_push_on_a_let_is_no_change`: a pure user fn `push` by UFCS on a let is refused.
- What reaches it: any `let x = f()`, match binding or `for` element of `Map` type changed in place.

## Acceptance

- The three cases are un-ignored and pass.
- The checker and the type checker give one E006 per site (no duplicates; `check --json` unchanged in shape).
- `task gate` green.
