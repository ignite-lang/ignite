---
format: aep.planning-md/3
id: story:data-value-model
kind: story
status: draft
title: std.data value model and the Json bridge
relations:
- decomposes: epic:data-formats
revision: 1
---
# Story: data-value-model

## Outcome

`std.data` declares `Value`, `Shape`, `Case`, `Tagging`, `Unknown`, `DecodeError`, `DataLimits` and `FormatError` as `docs/serde.md` sections 4, 5.1, 5.2 and 6 define them, and `std.json` converts between `Json` and `Value` both ways.

## Acceptance

- Inline tests: `from_value(to_value(j)) == j` for every `Json` variant, nested; `Bytes` gives base64 text and `Moment` its text in `from_value`.
- `std.json.parse_value`, `emit`, `emit_pretty` exist, with inline tests on both targets.
- `docs/layering.md` and `layering::` place `std.data` as a leaf of the base layer.
- `docs/slice.md` gains the section for `std.data`; `task gate` green.

## Scope

- `std/data.ignite` (new), `std/json.ignite`, `docs/layering.md`, `crates/ignite-check/tests/layering.rs`, `docs/slice.md`, `AGENTS.md` layout row. Inferred.
