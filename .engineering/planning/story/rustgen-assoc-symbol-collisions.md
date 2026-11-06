---
format: aep.planning-md/3
id: story:rustgen-assoc-symbol-collisions
kind: story
status: draft
title: Associated fn symbols never collide in generated Rust
revision: 1
---
# Story: rustgen-assoc-symbol-collisions

## Outcome

The Rust names of associated fns never collide with a free fn or another owner's associated fn.

## Context

- Found by `story:rustgen-generated-name-collisions` (PR 78, 2026-11-06), not fixed there: `assoc_symbol` (`crates/ignite-rustgen/src/lib.rs`, `{owner}__{name}`) gives the same name for a free fn `A__b` and `fn A.b`, and for owner `A_` with fn `_b` and owner `A` with fn `__b`.
- A different symbol scheme changes goldens (`crates/ignite-rustgen/tests/golden/`).

## Acceptance

- One program per collision above in `crates/ignite-rustgen/tests/names.rs`, red on main, green after; `CODEGEN_REVISION` bumped; `task gate` green.

## Out of Scope

- LLVM symbol names.
