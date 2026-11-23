---
format: aep.planning-md/3
id: story:resolver-hooks-recognized-by-full-signature
kind: story
status: draft
title: Resolver hooks are recognized by their full signature
revision: 1
---
# Story: resolver hooks are recognized by their full signature

## Outcome

The planner takes `admits` and `includes` of the binding type's module as resolver hooks only when their types are `(B, std.deploy.Member) -> Option<Text>` and `(B) -> List<Ref>`; fns of those names with other types are ordinary, and the plan is emitted with `resolution: null`.

## Why

IG-28, severity medium, class logic; observed with a released command (the maintainer review of 2026-11-23 against main at a1d1407e, v0.16.4 archive).

In a package with its own binding enum, ordinary `admits(Int, Int) -> Bool` and `includes(Text) -> Bool` caused E117 during planning; renaming only those helpers made the plan succeed with `resolution: null`.

`binding_fns` checks name and arity through `declares_free_fn` (`crates/ignite-eval/src/resolve.rs:39`, `:40`, `:90` to `:97`) and then calls the fns with deployment values. `docs/slice.md:2971` to `:2975` gives the two signatures. Read further at a1d1407e, not named by the review: `entries_fn` checks `entries` by name and arity the same way (`:63`).

## Acceptance

- A plan fixture, red first: the two helpers above give `resolution: null` and no E117.
- Controls: the existing deployment fixtures still resolve; one hook with the right type and the other with a wrong one is not resolvable; `task gate` green.

## Scope

- `crates/ignite-eval/src/resolve.rs:32` to `:48` (`binding_fns`, cited `:39`), `:90` (`declares_free_fn`, cited), `docs/slice.md:2971` (cited). Each read at a1d1407e, as the review states.
