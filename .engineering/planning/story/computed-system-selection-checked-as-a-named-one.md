---
format: aep.planning-md/3
id: story:computed-system-selection-checked-as-a-named-one
kind: story
status: draft
title: A system selected through a helper is checked and planned as a named one
revision: 1
---
# Story: a system selected through a helper is checked and planned as a named one

## Outcome

An assembly with `system = choose(Staging)`, where `choose` returns its `SystemRef` argument, plans a fully bound assembly and reports E110 for a missing binding in `ignite check`, as `system = Staging` does.

## Why

IG-29, severity medium, class logic; observed with a released command (the maintainer review of 2026-11-23 against main at a1d1407e, v0.16.4 archive).

Changing `system = Staging` to `system = choose(Staging)` made a fully bound plan fail with E114. With a required binding removed, `check` succeeded for the helper form while the direct form raised E110.

The type checker records the selected system only when the property is a bare name (`crates/ignite-types/src/typer/system.rs:1826` to `:1835`), otherwise an empty name (`:1836`), which also turns off the E108 key check. The model reports E114 when that name is absent (`crates/ignite-eval/src/model.rs:1293` to `:1300`) and looks the system up by the same optional name for checking (`:1373` to `:1376`). `docs/slice.md:2470`: `system = Terminal` is a `SystemRef`.

## Acceptance

- Fixtures, red first: the helper form with complete bindings plans as the direct form; with a required binding removed, `ignite check` gives E110 for both forms.
- Controls: the direct form unchanged; a helper that returns no system value gives the existing diagnostic; `task gate` green.

## Scope

- `crates/ignite-types/src/typer/system.rs:1826`, `crates/ignite-eval/src/model.rs:1293`, `:1373`, `docs/slice.md:2469` (cited). Each read at a1d1407e, as the review states.
