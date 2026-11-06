---
format: aep.planning-md/3
id: story:typed-program-json
kind: story
status: proposed
title: ignite emit --typed-json prints the typed program
summary: A versioned JSON document of the parsed and typed program, so tools written in Ignite can read Ignite
scope:
- confidence: inferred
  path: crates/ignite-types/src
- confidence: inferred
  path: crates/ignite/src/lib.rs
- confidence: inferred
  path: crates/ignite/src/main.rs
- confidence: inferred
  path: docs/slice.md
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-06T01:15:03Z", actor: "human:timo", revision: 6}
---
# Story: typed-program-json

## Outcome

`ignite emit --typed-json FILE` prints the parsed and typed program of FILE as one JSON document:
every declaration, statement and expression with its source position, and a type on every
expression, in a documented, versioned format (`ignite-typed/0.1`). Tools written in Ignite can then
read Ignite programs without a parser of their own.

## Context

- Needed by `ignite-lang/ignite-codespec` (`docs/design.md` section 4, milestone C2): its reader
  `codespec-read-ignite`, written in Ignite, maps the typed program to codespec's `cir/1`.
- Today `ignite emit` prints the generated Rust projects (`crates/ignite/src/main.rs:6`, `:913`).
- The typer keys its side tables by expression address (`crates/ignite-types/src/lib.rs:1319-1377`,
  `story:stable-node-ids`); a serialiser walking the AST while the `TypedProgram` is alive can read
  them without that story.

## Acceptance

- `ignite emit --typed-json` on every file under `std/` and `examples/` exits 0, and every
  expression object carries a `type`; a test walks the documents and checks both.
- The same file emitted twice gives the same bytes.
- The format is described in a new `docs/slice.md` subsection; a change that removes or
  reinterprets a key bumps `formatVersion`, an added key does not (as `ignite-model/0.1`).
- An Ignite test reads one emitted document with `std.json` and finds a named function's
  parameter types.
- `task gate` green.

## Out of Scope

- Stable node ids across edits (`story:stable-node-ids`).
- Any knowledge of `cir/1` in this repository.
