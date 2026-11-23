---
format: aep.planning-md/3
id: story:typed-json-ufcs-only-when-receiver-is-first-argument
kind: story
status: draft
title: Typed JSON marks `ufcs` only when the receiver is the first argument
relations:
- informed_by: story:typed-program-json
revision: 1
---
# Story: typed JSON marks `ufcs` only when the receiver is the first argument

## Outcome

In `ignite emit --typed-json`, `ufcs` is true only for a method call whose receiver is passed as the callee's first parameter: `Counter.zero()` (an associated call with no receiver argument) gives `ufcs: false`, `1.inc()` for a free `fn inc(n: Int)` gives `ufcs: true`.

## Why

IG-22, severity medium, class logic; observed with a released command (the maintainer review of 2026-11-23 against main at a1d1407e, v0.16.4 archive).

The emitter set `ufcs: true` for both calls above; a tool rebuilding calls from the documented schema would insert an argument into the first.

`call_parts` sets `ufcs` to `method && callee is Callee::Fn` (`crates/ignite/src/typed_json.rs:693`, `:694`). Typing an associated call `Owner.name(args)` records `Callee::Fn` and orders the arguments with no receiver (`ordered_args(expr, &q, 0, args)`, `crates/ignite-types/src/typer.rs:3032`, `:3033`). `docs/slice.md:9846`: `ufcs` is true for a method call `x.f(args)` that calls the free fn `f(x, args)`.

## Acceptance

- A typed-JSON golden test, red first: `Counter.zero()` false, `1.inc()` true.
- Controls: a plain call and a builtin method stay `ufcs: false`; `task gate` green.

## Scope

- `crates/ignite/src/typed_json.rs:688` to `:703` (`call_parts`; the review cites `:691`, the `callee` entry; the `ufcs` value is at `:694`), `crates/ignite-types/src/typer.rs:3033` (cited), `docs/slice.md:9846`.
