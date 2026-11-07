---
format: aep.planning-md/3
id: story:data-yaml
kind: story
status: draft
title: 'std.yaml: YAML 1.2.2 core schema, safe subset'
relations:
- decomposes: epic:data-formats
- depends_on: story:data-value-model
revision: 1
---
# Story: data-yaml

## Outcome

`std.yaml` parses and emits YAML 1.2.2 under the core schema, with the safe subset of `docs/serde.md` section 6.2: anchors, aliases, tags other than `!!binary`, complex keys and `<<` refused with a `FormatError` naming the line.

## Context

- Pusher case study G9, `docs/friction.md` row 68: rules written as Ignite data for lack of YAML.

## Acceptance

- The YAML test suite (`yaml/yaml-test-suite`) vendored under `docs/yaml/` with its licence; a test runs every case: each in-subset case gives the suite's expected JSON (through `std.json.from_value`), each case using a refused feature gives a `FormatError`, and the list of refused cases is a checked-in file.
- `yes`, `no`, `on`, `off` parse as text.
- `parse(emit(v)) == v` for generated values on both targets; `emit` quotes text that reads as another type (`"no"`, `"1.0"`, `"null"`).
- `DataLimits` bound depth and size, with a test each.
- `docs/slice.md` section; `task gate` green.

## Scope

- `std/yaml.ignite` (new), `docs/yaml/` (vendored), a test under `crates/ignite/tests/`, `docs/slice.md`, `AGENTS.md` layout row. Inferred.
