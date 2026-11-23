---
format: aep.planning-md/3
id: story:evaluated-bindings-key-of-another-system-is-e108
kind: story
status: active
title: A bindings key of another system is E108 whether written or computed
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T03:51:05Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-23T03:51:05Z", actor: "human:timo", revision: 3}
---
# Story: a bindings key of another system is E108 whether written or computed

## Outcome

An assembly whose `bindings` map is computed (returned by a helper) and holds a key that is not a component or resource of the assembly's system is refused with E108 by `ignite plan`, at the key's expression when its source position is known and at the `bindings` property otherwise; no key is dropped silently.

## Why

IG-09, severity high, class logic; observed with a released command (the maintainer review of 2026-11-23 against main at a1d1407e, v0.16.4 archive).

Synthetic reproducer: a helper returning a map with one member of the selected system and one of a second system. `ignite check` succeeded; `ignite plan` showed two raw bindings and one resolved entry. The same map written as a literal raised E108.

The resolver skips every evaluated key whose id is not a member of the selected system with `continue` (`crates/ignite-eval/src/resolve.rs:186` to `:192`). E108 comes from the type checker (`crates/ignite-types/src/typer/system.rs:611` to `:625`), which sees only keys written in the source. `docs/slice.md:2471`: every `bindings` key must be a component or resource of that system, otherwise E108.

## Acceptance

- A plan fixture, red first: the helper form gives E108 naming the foreign key; a helper key naming a member that does not exist in the selected system gives E108 too (the same `continue`).
- Controls: the literal form stays E108 at the key; a helper map with only members of the selected system plans as before.
- `task gate` green.

## Scope

- `crates/ignite-eval/src/resolve.rs:182` to `:192` (cited `:186`), `crates/ignite-types/src/typer/system.rs:611` to `:625` (cited `:613`), `docs/slice.md:2471`. Each read at a1d1407e, as the review states.
