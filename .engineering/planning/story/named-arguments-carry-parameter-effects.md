---
format: aep.planning-md/3
id: story:named-arguments-carry-parameter-effects
kind: story
status: implemented
title: A call with named arguments needs the same effects as the positional call
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T03:51:00Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-23T03:51:00Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-11-23T21:50:04Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
# Story: a call with named arguments needs the same effects as the positional call

## Outcome

For an effectful fn whose `requires` names a parameter (`out.write`), a call that passes that argument by name, in declared order or reordered, gives the caller the same inferred effects and the same E016 diagnostics as the positional call.

## Why

IG-02, severity high, class security; traced in source only (the maintainer review of 2026-11-23 against main at a1d1407e). No program was run.

`callee_effects` (`crates/ignite-check/src/lib.rs:2552`) maps a requirement `q.op` to the position of `q` among the declared parameters and takes `args.get(index)` from the call's arguments in source order (`:2564`). A reordered named call therefore charges the effect to the wrong argument. The provenance walker `roots` (`:2261`) has no arm for `ExprKind::NamedArg`; its fallthrough `_ => BTreeSet::new()` (`:2326`) gives a named argument no roots even in declared order, so the requirement maps to nothing: a caller without `requires` infers no entry for it, and a caller with `requires` that lacks it gets no E016. The type checker does put named arguments in parameter order (`crates/ignite-types/src/typer.rs:469`, `:2375`); the checker does not use that order. `docs/slice.md:423` states the rule: each entry `q.op` of the callee becomes `r.op` for each root `r` of the argument passed for `q`.

## Acceptance

- A test, red first: a callee with two stream parameters and `requires out.write`, called positionally, as `f(out = s, n = 1)` and as `f(n = 1, out = s)`. All three report the same E016 in a caller whose `requires` lacks `s.write`, and all three infer `s.write` in a caller without `requires`.
- Controls: a requirement on an ambient capability (`stdout.write`) is unchanged; the method form `a.f(..)` keeps the receiver as the first argument; a call of a fn value with a parameter-scoped requirement gives the same result for named and positional arguments, or names stay refused there as today.
- `task gate` green.

## Scope

- `crates/ignite-check/src/lib.rs:2552` to `:2573` (`callee_effects`). The review cites `:2561`, which is the closing `});` of the parameter lookup; the indexing it describes is at `:2564`.
- `crates/ignite-check/src/lib.rs:2261` to `:2328` (`roots`, fallthrough at `:2326`, as cited), `:3849` (the direct call site, as cited, which passes the source-order arguments at `:3853` and `:3854`).
- Not cited by the review, same pattern: `fn_value_effects` indexes source-order arguments at `:3579`.
- `docs/slice.md:423` (the rule; the review cites `:405`, the start of section 5.1).
