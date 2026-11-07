---
format: aep.planning-md/3
id: story:data-yaml
kind: story
status: draft
title: 'std.yaml: YAML 1.2.2 core schema, safe subset'
relations:
- decomposes: epic:data-formats
- depends_on: story:data-value-model
revision: 2
---
# Story: data-yaml

## Outcome

`std.yaml` reads (`read`, `read_with`, `read_all` for several documents in one stream) and writes (`write`) YAML 1.2.2 under the core schema, with the safe subset of `docs/serde.md` section 6.2: anchors, aliases, tags other than `!!binary`, complex keys and the `<<` merge key refused with a `FormatError` naming the line.

## Context

- Pusher case study G9, `docs/friction.md` row 68: rules written as Ignite data for lack of YAML.

## Acceptance

- The YAML test suite (`yaml/yaml-test-suite`) vendored under `docs/yaml/` with its licence; a test runs every case. Each case outside the refused features gives the suite's expected JSON (through `std.json.of_data`). Each refused case gives a `FormatError`. The refused list is a checked-in file, and the test fails when a listed case uses none of the five refused features (it greps the case's input for `&`, `*`, `!` other than `!!binary`, `? ` and `<<`).
- `read_all` of a stream with three documents gives three `Value`s.
- `yes`, `no`, `on`, `off` read as text.
- `read(write(v)) == v` for 200 values from a seeded generator over every `Value` variant, on both targets; `write` quotes text that reads as another type (`"no"`, `"1.0"`, `"null"`).
- 10 suite cases read to the same `Value` in the evaluator and on both targets.
- `DataLimits` bound depth and size, a test each.
- `docs/slice.md` section; `task gate` green.

## Scope

- Own: `std/yaml.ignite` (new), `docs/yaml/` (vendored), a test file under `crates/ignite/tests/`.
- Shared with `story:data-toml` and `story:data-codec-combinators` in one wave: `docs/layering.md` and `crates/ignite-check/tests/layering.rs` (the `std.yaml` row, layering rule 4), `docs/slice.md`, `AGENTS.md` layout table, `crates/ignite/tests/main.rs` (one `mod` line).
