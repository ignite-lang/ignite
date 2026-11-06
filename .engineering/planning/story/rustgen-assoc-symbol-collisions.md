---
format: aep.planning-md/3
id: story:rustgen-assoc-symbol-collisions
kind: story
status: active
title: Associated fn symbols never collide in generated Rust
scope:
- confidence: cited
  path: crates/ignite-rustgen/src/lib.rs
- confidence: cited
  path: crates/ignite-rustgen/tests/names.rs
- confidence: cited
  path: crates/ignite/src/apply/plan.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-06T23:50:47Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-11-06T23:50:47Z", actor: "human:timo", revision: 5}
---
# Story: rustgen-assoc-symbol-collisions

## Outcome

The Rust names of associated fns never collide with a free fn or another owner's associated fn.

## Context

- Found by `story:rustgen-generated-name-collisions` (PR 78, 2026-11-06), not fixed there: `assoc_symbol` (`crates/ignite-rustgen/src/lib.rs`, `{owner}__{name}`) gives the same name for a free fn `A__b` and `fn A.b`, and for owner `A_` with fn `_b` and owner `A` with fn `__b`.
- A different symbol scheme changes goldens (`crates/ignite-rustgen/tests/golden/`).

## Acceptance

## Acceptance

Scoped 2026-11-07 on main df7be28c: `assoc_symbol` (`crates/ignite-rustgen/src/lib.rs:1813`) is plain `{owner}__{name}`; free fns go through `ident` (`:1473`).

- `crates/ignite-rustgen/tests/names.rs` gains three programs, each failing `ignite build --target rust` (or returning the wrong value) on main and correct after, a run showing every fn returns its own value: a free `fn A__b` beside `fn A.b`; `fn A_._b` beside `fn A.__b`; `fn A__b.c` beside `fn A.b__c`.
- No file under `crates/ignite-rustgen/tests/golden/` changes (the scoped scheme: `_` in owner and name escaped as `_0` before joining with `__`; `ident` escapes any name containing `__`).
- `docs/slice.md` 13.8 names the Rust scheme; `CODEGEN_REVISION` above main's; `task gate` green.

## Out of Scope

- LLVM symbol names.
