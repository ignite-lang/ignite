---
format: aep.planning-md/3
id: story:eval-closures-and-members
kind: story
status: draft
title: Closures in the system model and members derived by a kind
relations:
- decomposes: epic:x-web
scope:
- confidence: inferred
  path: crates/ignite-eval/src
- confidence: inferred
  path: crates/ignite/src/run
- confidence: inferred
  path: crates/ignite/tests/golden
- confidence: inferred
  path: crates/ignite/tests/main.rs
- confidence: inferred
  path: docs/slice.md
- confidence: inferred
  path: std/deploy.ignite
revision: 5
---
# Story: eval-closures-and-members

## Outcome

The system model holds closures and function values in component properties, encoded as the creating function and its captures, so a route table is a property (X3); and a component kind's `members` hook returns members the evaluator adds to the model, shown by `ignite plan` and `ignite explain` as derived and operated by `ignite run` like written ones (X9).

## Context

- web §5 rows X3 (today E118 "a closure cannot be encoded in the model", `Value::Closure` in `crates/ignite-eval/src/value.rs`) and X9 (as `std.control.companions` adds machines, slice §16.12).
- web §6.5 and §7: derived members are listed with `derived by <component>`; a member declared by hand replaces the derived one of the same name.
- Both are evaluator work and name no protocol; `story:web-collection` and `story:web-service-calls` are their first users.

## Acceptance

- A component property holding a closure that captures a resource checks with no E118; `ignite plan` encodes it as the creating fn's path and its captures (golden); two equal closures encode equally, so the plan is canonical (test runs `plan` twice and compares bytes).
- A fixture kind with a `members` hook returning one probe and one contract: `ignite plan` lists both with `derived by Sys.c` (golden); `ignite explain FILE Sys.c.reachable` names the hook; `ignite run --clock manual` schedules the derived probe and reports the contract's standing (end-to-end test, one target); a hand-declared member named like a derived one replaces it, and the plan shows it as written.
- An assembly binds a derived member like a written one: `Sys.c.health = scheduledProbeHost()` in a fixture assembly resolves, `ignite plan --assembly` shows the binding (golden), and binding a derived member name the hook does not produce is E110 to E114 as for a written one (fixture).
- A `members` hook that returns a member whose name collides with a resource is E120 (fixture).
- `ignite replay` of a run with a derived member compares clean.
- `docs/slice.md` section 15 and 16 additions; `task gate` green.

## Scope

- `crates/ignite-eval/src` (`value.rs` encoding, model construction, explain).
- `std/deploy.ignite` (the `members` hook vocabulary) and the fixture kind under `crates/ignite/tests/fixtures/`.
- `crates/ignite/src/run/` (derived members scheduled), `crates/ignite/tests/golden/`, `crates/ignite/tests/`.
- `docs/slice.md`. Inferred from web §8 rows 5 and 7.
- Ordering: may run beside `story:web-core`; they share `docs/slice.md` and `crates/ignite/tests/main.rs` (each adds a `mod` line and its own section), merged on the wave's integration branch; before `story:route-annotations`, `story:given-and-defaults` and the rest of `epic:x-web`, which edit `crates/ignite-eval/src` after it.
