---
format: aep.planning-md/3
id: story:llvm-field-default-symbols-apart-from-associated-fns
kind: story
status: draft
title: An LLVM field default and an associated function of the same name get different symbols
revision: 1
---
# Story: an LLVM field default and an associated function of the same name get different symbols

## Outcome

A record with a defaulted field `count` and a zero-argument associated function `Item.count()` builds and runs on the LLVM target, as it does on the Rust target.

## Why

IG-25, severity medium, class logic; observed with a released command (the maintainer review of 2026-11-23 against main at a1d1407e, v0.16.4 archive).

A Rust native test reading both values passed; the LLVM target rejected the duplicate definition.

`default_symbol` names a field default `<module>.<Record>.<field>` (`crates/ignite-llvm/src/lower.rs:94` to `:96`) and defines it at `crates/ignite-llvm/src/lib.rs:409`, `:410`. An associated fn is defined under `symbol` of its qualified name, `<module>.<Record>.<fn>` (`:414`, `:419`).

## Acceptance

- A native test on both targets, red first on LLVM: the record and function above, reading both values.
- Controls: the existing field-default tests; `task gate` green; `CODEGEN_REVISION` bumped if generated code changes.

## Scope

- `crates/ignite-llvm/src/lower.rs:94`, `crates/ignite-llvm/src/lib.rs:409`, `:419` (cited). Each read at a1d1407e, as the review states.
