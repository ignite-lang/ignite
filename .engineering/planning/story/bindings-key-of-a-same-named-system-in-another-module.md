---
format: aep.planning-md/3
id: story:bindings-key-of-a-same-named-system-in-another-module
kind: story
status: draft
title: A bindings key of a same-named system in another module
revision: 2
---
# Story: a bindings key of a same-named system in another module

## Outcome

An assembly over system `S` whose bindings hold a key of another module's system that is also named `S` gets E108 from `ignite check` and `ignite plan`, and the E108 message names the module of the key's system, so it does not say that `S.member` is not a member of `S` when it is one of the other `S`.

## Why

Found by the adversary pass on the unit of `story:evaluated-bindings-key-of-another-system-is-e108` (wave 16, 2026-11-23); older than that unit, by reading `crates/ignite-eval/src/resolve.rs` on main. Observed: a sibling module `sz.helper` declares its own `system S { ingress, egress }` and a helper returning keys of that `S`; the program's assembly over its own `S` uses the helper; `check` exits 0 and the keys resolve as the program's members. Member ids are bare `System.member` with no module (`member_id`, the `strip_prefix(system.name)` match in `resolve.rs`). No program of the repository does this.

## Acceptance

- The program of the adversary case `a_key_of_a_sibling_modules_system_of_the_same_name_is_e108`, in commit `82ca50d2` of branch `wave/16-bindings-e108`, file `crates/ignite/tests/bindings_e108_adversary.rs` (removed from the tree so the count of ignored tests stays at its bar), comes back as a test: `check` and `plan` exit 1 with one E108 per key, and the message names the key's module.
- Control: keys of the program's own `S` resolve as before.

## Scope

- `crates/ignite-eval/src/resolve.rs` (`member_id`, member matching), the type checker's member ids if they share the form. Inferred.
