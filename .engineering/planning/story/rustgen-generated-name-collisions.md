---
format: aep.planning-md/3
id: story:rustgen-generated-name-collisions
kind: story
status: draft
title: Generator-invented Rust names never collide with program names
summary: Escape __ locals, x_endpoint, default_field and extern-crate names in the Rust target
relations:
- derived_from: story:rustgen-builtin-and-role-shadowing
scope:
- confidence: inferred
  path: crates/ignite-rustgen/src
revision: 2
---
# Story: rustgen-generated-name-collisions

## Outcome

No name the Rust generator invents can collide with a name an Ignite program declares, so every
well-typed program compiles through cargo.

## Context

- Found while fixing `story:rustgen-builtin-and-role-shadowing` (PR 16, 2026-11-04); not tested,
  read from `crates/ignite-rustgen/src/lib.rs`:
  - generator locals `__m`, `__x`, `__a0`, `__pN`, `__spawn_N`, `__ignite_*` against an Ignite
    identifier that starts with `__` (Ignite identifiers can be any ASCII name);
  - `endpoint_variable` builds `{x}_endpoint`, against a resource named `x_endpoint`;
  - `default_{field}` against a user associated fn of that name;
  - a package whose crate name shadows an extern crate (`std`, `core`, `ignite_io`, `ignite_std`).
- PR 16's `ident` escape (a trailing `_`, one to one) is the pattern to extend.

## Acceptance

- One golden or end-to-end case per collision above, red before the fix and green after;
  `CODEGEN_REVISION` bumped; `task gate` green.

## Out of Scope

- The LLVM target's symbol names.
