---
format: aep.planning-md/3
id: story:effectful-map-escape
kind: story
status: implemented
title: No effect passes List.map unchecked through an untyped local
scope:
- confidence: cited
  path: crates/ignite-check/src/lib.rs
- confidence: cited
  path: crates/ignite-types/src/typer.rs
- confidence: cited
  path: crates/ignite-types/tests
- confidence: cited
  path: docs/slice.md
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-07T02:27:31Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-11-07T02:27:31Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-11-07T04:58:45Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---
# Story: effectful-map-escape

## Outcome

No effect passes `List.map` unchecked: a local of effectful fn type that `ignite-check` cannot type is E042 at `map`, as at a direct call, and the coverage check of `list_map` has a test on every path.

## Context

- Adversary review of PRs 72 and 83 (2026-11-07); branch `test/adv-effectful-map` at `55e53766` adds `crates/ignite-types/tests/effectful_map_adversary.rs`, 4 cases red on main.
- F1 (introduced by PR 83, blocker): `let g = make()`, `let g = hold().f` or a `Some(g)` pattern binding of effectful fn type passed to `map` checks clean. `map_fn_value` charges nothing because `local.ty` is `None` (`crates/ignite-check/src/lib.rs:2817`); `map_fn_effects` accepts the `EffectFn` (`crates/ignite-types/src/typer.rs:3488`). A fn without `requires` performs `clock.read` with no E016 at its caller; inside a closure passed to `map` under `requires [stdout.write]`, `row.map(g)` performs `clock.read` with no diagnostic. Calling `g(n)` directly is E042.
- F2 (introduced): the mutant `map_effects_fit(..).then_some(..)` → `Some(..)` at `typer.rs:3465` survives the old suite; the branch's `an_uncovered_effectful_fn_mapped_into_an_unannotated_let_is_e017` kills it.

## Acceptance

- The 6 cases of `test/adv-effectful-map` are on main and pass: the 3 locals above and the closure case give E042 (or are charged correctly), the control and the mutant guard pass.
- `docs/slice.md` 20.4.3 names the rule; `task gate` green.
