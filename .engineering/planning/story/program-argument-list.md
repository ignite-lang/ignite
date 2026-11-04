---
format: aep.planning-md/3
id: story:program-argument-list
kind: story
status: draft
title: Read the program's argument list
summary: A program reads its whole command line as a list of Text, so a CLI with subcommands and positional arguments can be written
relations:
- decomposes: epic:playground-substrate
- depends_on: story:program-arguments
scope:
- confidence: inferred
  path: crates/ignite-io/src/lib.rs
- confidence: inferred
  path: crates/ignite-rt/src
- confidence: inferred
  path: crates/ignite-types/src
- confidence: inferred
  path: std/process.ignite
revision: 2
---
# Story: program-argument-list

## Outcome

A program reads its whole command line as a list of `Text`, so a command-line tool with
subcommands and positional arguments (`todo add "buy milk"`, `todo done 3`) can be written in
Ignite on both targets.

## Context

- Found by the ignite-ess examples (2026-11-04): `examples/todo-cli/todo-cli.yaml` in the sibling
  ignite-ess repository is an `ess-cli/2` binding with the commands `describe add list show edit
  done remove`, meant to compile to a native binary for LLM agents; its `docs/design.md` section 10
  records this as gap I1.
- Slice 17 (`docs/slice.md` 21.6 on `feat/slice17-processes-files`) gives `Setting`, one
  `--flag V` per declaration, read by the generated `main`; unknown arguments are ignored and an
  argument-parsing library beyond `Setting` is out of scope (21, the out-of-scope list). No
  subcommand or positional argument reaches the program.
- The capability follows slice 17's rule P3: a handle in the program's state, needing an effect,
  test-only constructor outside tests E051, as `Setting.of(..)`.

## Acceptance

- A fixture on both targets started as `<exe> add "buy milk" --port 0` prints its arguments as
  `add`, `buy milk`, `--port`, `0`, in order, without the executable's name.
- A test constructs the handle from a literal list and runs the same parse inline on both targets.

## Out of Scope

- A parsing library in `std` (subcommand tables, help text); the program or `std.cli` builds it
  on the list.
