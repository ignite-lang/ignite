---
format: aep.planning-md/3
id: story:ignite-fmt
kind: story
status: proposed
title: ignite fmt and a format check
revision: 2
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T01:26:00Z", actor: "human:timo", revision: 2}
---
# Story: ignite-fmt

## Outcome

`ignite fmt FILE…` rewrites Ignite source into one canonical layout, and `ignite fmt --check FILE…` exits 1 naming each file that would change, so a project can hold a format gate.

## Context

- Case study gap G17 (`docs/research/case-study-gaps.md`, friction row 73): the case study's format task is a no-op; every other implementation has a formatter gate.

## Acceptance

- `ignite fmt` is idempotent: `fmt(fmt(x)) == fmt(x)` for every file under `std/` and `examples/`, and the formatted files parse to the same AST (spans aside).
- Comments survive at their position, in fixtures with comments before, after and inside items, arms and lists.
- `ignite fmt --check` exits 0 on formatted files and 1 with the file list otherwise; `task check` runs it over `std/` and `examples/`.
- `docs/slice.md` section with the layout rules; `task gate` green.

## Scope

- `crates/ignite-syntax` (a printer from the AST with comments), `crates/ignite/src` (the command), `Taskfile.yml`, `std/`, `examples/` (reformatted once), `docs/slice.md`. Inferred.
