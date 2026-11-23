---
format: aep.planning-md/3
id: story:protocol-roles-infer-local-types-as-fns-do
kind: story
status: draft
title: Protocol roles infer local types as ordinary functions do
revision: 1
---
# Story: protocol roles infer local types as ordinary functions do

## Outcome

Inside a protocol role, an empty list bound without annotation and later given `push(value)` with `value: Int` checks as it does inside an ordinary fn; a list nothing constrains stays E042.

## Why

IG-17, severity medium, class logic; observed with a released command (the maintainer review of 2026-11-23 against main at a1d1407e, v0.16.4 archive).

The same body passed inside an ordinary fn and failed with E042 inside a protocol role.

In `bodies`, tests and fns are typed through `with_local_inference` (`crates/ignite-types/src/typer.rs:1719`, `:1722`); a role calls `function` directly (`:1735`). `docs/slice.md:2069` (row 3) states the later-use inference for a body, without excluding roles.

## Acceptance

- A test, red first: the role body checks and its protocol runs on both targets.
- Controls: an unconstrained empty list in a role stays E042; `task gate` green.

## Scope

- `crates/ignite-types/src/typer.rs:1722`, `:1735` (cited), `docs/slice.md:2069`. Each read at a1d1407e, as the review states.
