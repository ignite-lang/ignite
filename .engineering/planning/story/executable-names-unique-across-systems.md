---
format: aep.planning-md/3
id: story:executable-names-unique-across-systems
kind: story
status: implemented
title: Two components of different systems cannot share one executable name
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T03:51:04Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-23T03:51:05Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-11-23T21:50:07Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
# Story: two components of different systems cannot share one executable name

## Outcome

A program in which components of different systems produce the same executable name is refused by `ignite check` with E020, before any output is written, as two components of one system are today.

## Why

IG-08, severity high, class logic; observed with a released command (the maintainer review of 2026-11-23 against main at a1d1407e, v0.16.4 archive).

Synthetic reproducer: two systems in one file, each with a component named `tool` with different field values. `ignite check` succeeded and an LLVM build printed the destination `bin/tool` twice; only one file can hold both.

The list of executable names is created inside the typing of one system (`crates/ignite-types/src/typer/system.rs:1265`, in `system` from `:1208`) and compared there (`:1284` to `:1293`). The LLVM writer joins every main of the program to one binary directory (`crates/ignite/src/llvm_build.rs:660`, `:661`). `docs/slice.md:2645` and `:2646` say two components with one executable name are E020, without limiting that to one system.

## Acceptance

- A fixture, red first: two systems each with a component `tool` give E020 at the second, naming the first; a second fixture where one component sets `executable = "x"` and a component of the other system is named `x` gives E020 too.
- Controls: two systems with distinct executable names build both executables on both targets; one system with a repeated name stays E020.
- `task gate` green.

## Scope

- `crates/ignite-types/src/typer/system.rs:1265`, `:1284` (cited), `crates/ignite/src/llvm_build.rs:660` (cited), `docs/slice.md:2641` to `:2647`. Each read at a1d1407e, as the review states.
- `story:executable-names-stay-inside-the-build-directory` validates the same names.
