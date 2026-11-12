---
format: aep.planning-md/3
id: story:glob-imports-resolve-by-receiver
kind: story
status: draft
title: A name two globs provide resolves by the receiver's type
revision: 1
---
# Story: a name two globs provide resolves by the receiver's type

## Outcome

With `import std.option.*` and `import std.result.*` in one module, `o.unwrap_or(d)` on an `Option` and `r.unwrap_or(d)` on a `Result` both check: a method-call form picks the fn whose first parameter accepts the receiver.

## Why

Requested on 2026-11-12 by the author of a command-line tool written in Ignite, which declares its own `unwrap_or` in one module for this reason. `std/option.ignite` and `std/result.ignite` both declare `unwrap_or` and `map`; a name two globs provide is E041 at its use (`docs/slice.md` section 4, `crates/ignite-check/src/lib.rs`). `docs/friction.md` row 21 is open since 2026-11-01.

## Acceptance

- `x.name(..)` where two glob imports provide `name`: the fn whose first parameter type accepts `x` is called; none or more than one accepting is E041, and the diagnostic lists the candidates with their first parameter types.
- The plain call `name(x, ..)` on such a name stays E041 (the rule is for the method-call form only), unless the design step decides otherwise and says why.
- A fn declared in the module itself still wins over both globs, as today.
- One differential program with both std modules imported by glob runs on the evaluator and both targets.
- Row 21 of `docs/friction.md` moves to solved; `docs/slice.md` section 4 states the rule; `task gate` green.

## Scope

- `crates/ignite-check/src/lib.rs` (E041), `crates/ignite-types/src/typer.rs` (method calls on user fns), `docs/slice.md` section 4, `docs/friction.md`. Inferred.

## Not in this story

- Overloading: two fns of one name in one module.
- A module-qualified call (`story:module-qualified-calls`).
