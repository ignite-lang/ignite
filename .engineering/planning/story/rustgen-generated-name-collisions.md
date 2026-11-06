---
format: aep.planning-md/3
id: story:rustgen-generated-name-collisions
kind: story
status: active
title: Generator-invented Rust names never collide with program names
summary: Escape __ locals, x_endpoint, default_field and extern-crate names in the Rust target
relations:
- derived_from: story:rustgen-builtin-and-role-shadowing
scope:
- confidence: inferred
  path: crates/ignite-rustgen/src
- confidence: cited
  path: crates/ignite-rustgen/src/lib.rs
- confidence: cited
  path: crates/ignite-rustgen/tests
- confidence: cited
  path: crates/ignite-rustgen/tests/golden
- confidence: cited
  path: crates/ignite/src/apply/plan.rs
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-06T15:04:13Z", actor: "human:timo", revision: 6}
- {from: "proposed", to: "active", at: "2026-11-06T15:04:13Z", actor: "human:timo", revision: 7}
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

## Acceptance

Scoped 2026-11-06 from main 317955736d: none of the four is guarded (`crates/ignite-rustgen/src/lib.rs:1463` `ident`, `:6344` `endpoint_variable`, `:1476` `default_fn`, `:1315` `crate_name`).

- One case per collision, each its own program: a user name `__x` (and one each of `__m`, `__a0`, `__p0`, `__spawn_0`, `__ignite_x` in the same program), a resource `x_endpoint` beside a resource `x`, a user associated fn `default_f` on a record with a defaulted field `f`, packages named `std`, `core`, `ignite_io`, `ignite_std`.
- Each case builds with `ignite build --target rust` and, where the collision compiles silently, a test asserts the program's output (the value bound is the user's, not the generator's). Red on main, green after.
- Existing goldens change only where a user name is escaped.
- `CODEGEN_REVISION` bumped; `task gate` green.

## Out of Scope

- The LLVM target's symbol names.
