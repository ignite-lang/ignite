---
format: aep.planning-md/3
id: story:rustgen-reserved-names
kind: story
status: draft
title: Program names that shadow Rust primitives, crate roots or generator locals still build
revision: 1
---
# Story: rustgen-reserved-names

## Outcome

A program may name a record, type parameter, module, listener or protocol after a Rust primitive, a crate root or a generator local and still build through cargo.

## Context

- Adversary review of PRs 78 and 81 (2026-11-07); branch `test/adv-rustgen-names` at `5b0a546a` adds 8 cases to `crates/ignite-rustgen/tests/names.rs`, 7 red on main:
  - a record named `i64` (E0072, E0308) and a type parameter named `bool` (E0308): `ident` (`crates/ignite-rustgen/src/lib.rs:1478`) leaves Rust primitive names alone;
  - a record named `std` (E0223), sibling modules named `std` and `ignite_io` (E0433, E0425): generated paths start at bare `std::` and `ignite_io::` (`:1577`); `crate_name` escapes only the package name;
  - a listener `A_` beside `fn A.endpoint`: `endpoint_variable` (`:6373`) gives `A__endpoint`, the `assoc_symbol` of `A.endpoint` (E0618);
  - a protocol named `port`: `main`'s local `port` (`:6482`) becomes a struct pattern against the unit struct `use crate::*` brings in (E0308, E0599).
- The eighth case (free `fn A__b` beside `fn A.b_`) is green and guards a mutant of `assoc_symbol` the PR 81 cases miss (inferred, mutant not run).

## Acceptance

- The 8 cases of `test/adv-rustgen-names` are on main and pass.
- Generated paths to the standard library and the substrate cannot be shadowed by a program name (for example `::std::`, `::ignite_io::`), and generator-invented locals in `main` use names a program cannot declare.
- `CODEGEN_REVISION` bumped; goldens changed only where a path or local is renamed; `task gate` green.
