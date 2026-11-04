---
format: aep.planning-md/3
id: story:program-arguments
kind: story
status: draft
title: Programs read their own arguments
summary: Read command-line arguments beyond --port and --ref, so a server takes its root and tool paths at launch
relations:
- decomposes: epic:playground-substrate
scope:
- confidence: inferred
  path: crates/ignite-io/src
- confidence: inferred
  path: crates/ignite-rt/src
- confidence: inferred
  path: std
revision: 2
---
# Story: program-arguments

## Outcome

An Ignite program reads its own command-line arguments, so a server takes its root folder and the
compiler's path at launch instead of compiling them in.

## Context

- Gap P8 of the playground session (2026-11-04, compiler `9578530`): the substrate reads only
  `--port` and `Endpoint.of`'s `--ref` (`crates/ignite-io/src/lib.rs:346`).
- Under `epic:playground-substrate`; the arguments reach handlers the way P3 requires (as values
  passed in, not ambient statics).

## Acceptance

- An inline test or end-to-end test on both targets starts a program with `--root DIR --tool PATH`
  and reads both values; `--port` and `--ref` keep their meaning.

## Out of Scope

- An argument-parsing library beyond what `std.cli` declares.
