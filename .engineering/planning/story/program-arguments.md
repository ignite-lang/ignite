---
format: aep.planning-md/3
id: story:program-arguments
kind: story
status: implemented
title: Programs read their own arguments
summary: Read command-line arguments beyond --port and --ref, so a server takes its root and tool paths at launch
relations:
- decomposes: epic:playground-substrate
- depends_on: story:slice-17-frontend
- depends_on: story:process-spawn
scope:
- confidence: cited
  path: crates/ignite-io/src/lib.rs
- confidence: cited
  path: crates/ignite/src/apply/plan.rs
- confidence: cited
  path: crates/ignite/src/run
- confidence: cited
  path: crates/ignite/tests/slice17.rs
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-05T06:27:17Z", actor: "human:timo", revision: 5}
- {from: "proposed", to: "active", at: "2026-11-05T06:27:17Z", actor: "human:timo", revision: 6}
- {from: "active", to: "implemented", at: "2026-11-05T07:34:50Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"test_result":1}}}
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
- Contract: `docs/slice.md` section 21 on `feat/slice17-processes-files`, unit U4 of 21.12, rows 18, 26 and 32 of 21.0 (the `Setting` handle; open question Q6 confirmed: it fits this story); first failing test `slice17::rust::settings_read_their_flags`; after `story:slice-17-frontend` and `story:process-spawn`.

## Acceptance

- An inline test or end-to-end test on both targets starts a program with `--root DIR --tool PATH`
  and reads both values; `--port` and `--ref` keep their meaning.

## Out of Scope

- An argument-parsing library beyond what `std.cli` declares.
