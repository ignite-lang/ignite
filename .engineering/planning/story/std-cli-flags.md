---
format: aep.planning-md/3
id: story:std-cli-flags
kind: story
status: draft
title: std.cli parses declared flags
relations:
- informed_by: story:program-argument-list
revision: 1
---
# Story: std.cli parses declared flags

## Outcome

A command-line program declares its flags as a table (name, kind, default, the modes it applies to) and `std.cli` parses the argument list against it, with the usage text and the errors a hand-written parser would give.

## Why

Requested on 2026-11-12 by the author of a command-line tool written in Ignite: it parses its own flags in two functions, one per mode, for `--x=v` and `--x v`, booleans, integers and flags required in one mode only. `std/cli.ignite` has positional `Argument` and `Command` with `usage`. A `Setting` holds `Text` only and must be present at launch (`docs/slice.md` 21.6). Row 45 of `docs/friction.md` is related: the substrate's own flags show up in a `CommandLine`.

## Acceptance

- A flag is declared with a name, a kind (`Bool`, `Int`, `Text`, a repeated `Text`), a default or "required", and a help line; a command declares the flags it takes.
- `parse(command, arguments)` accepts `--name value`, `--name=value`, `--flag` and `--no-flag` for a `Bool`, and `--` ending the flags; it returns the values and the positional rest, or an error that names the flag: unknown, a missing value, not an integer, required and absent, given twice where not repeated.
- The substrate's own flags (row 45 of `docs/friction.md`) are skipped by `parse` without the program naming them; the row moves to solved or its remainder is stated.
- `usage` lists the flags of a command with kind, default and help.
- A property test: every argument list `usage` describes parses, and an argument list written back from the parsed values parses to the same values.
- An example program with two modes under `examples/` uses it; `ignite test` passes on both targets; `docs/slice.md` section; `task gate` green.

## Scope

- `std/cli.ignite`, `examples/`, `docs/slice.md`, `docs/friction.md`. Inferred.

## Not in this story

- Environment variables and configuration files as sources of a flag.
- Shell completion.
