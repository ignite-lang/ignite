---
format: aep.planning-md/3
id: story:duplicate-names-in-parameters-and-patterns-are-refused
kind: story
status: draft
title: A repeated name in a parameter list or a pattern is refused
revision: 1
---
# Story: a repeated name in a parameter list or a pattern is refused

## Outcome

A fn, role or closure with two parameters of one name, and a pattern that binds one name twice (`(x, x)`), are refused by `ignite check`; a later statement that shadows a parameter stays valid.

## Why

IG-19, severity medium, class logic; observed with a released command (the maintainer review of 2026-11-23 against main at a1d1407e, v0.16.4 archive).

A fn with two parameters named `x` passed `ignite check`; the emitted Rust failed with E0415. Destructuring a pair into `(x, x)` also passed; the emitted Rust failed with E0416.

The checker binds a name by inserting it into the current scope map (`crates/ignite-check/src/lib.rs:2156` to `:2165`), which replaces an earlier entry silently. `check_fn` (`:1910`) binds the parameters and `pattern` (`:4067`) the pattern names through it.

## Acceptance

- Fixtures, red first: duplicate parameters in a fn, a closure and a role; `(x, x)` in a `let` and in a `match` arm; each is a diagnostic at the second occurrence.
- Controls: `let x = ..` after a parameter `x`; the same name in two different match arms; an or-pattern keeps its E049 rule; `task gate` green.

## Scope

- `crates/ignite-check/src/lib.rs:2157` (the insert, cited), `:1910` (`check_fn`, cited), `:4067` to `:4080` (`pattern`, cited `:4068`). Each read at a1d1407e, as the review states.
