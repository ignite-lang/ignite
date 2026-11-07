---
format: aep.planning-md/3
id: story:data-value-model
kind: story
status: draft
title: std.data value model and the Json bridge
relations:
- decomposes: epic:data-formats
revision: 2
---
# Story: data-value-model

## Outcome

`std.data` declares `Value`, `Shape`, `Case`, `Tagging`, `Unknown`, `DecodeError`, `DataLimits` and `FormatError` as `docs/serde.md` sections 4, 5.1, 5.2 and 6 define them, and `std.json` reads and writes `Value` (`read`, `read_with`, `write`, `write_pretty`) and converts `Json` both ways (`to_data`, `of_data`).

## Acceptance

- Inline tests: `of_data(to_data(j)) == j` for every `Json` variant, nested; `of_data` gives base64 text for `Bytes` and the text for `Moment`.
- Inline tests: `Shape.default()` has `keys = Same`, `variants = Same`, `tagging = External`, `unknown = Ignore`, no renames; `DecodeError.describe()` gives `$.a[1].b: expected Int, found Text`; a `FormatError` from `std.json.read` of a bad document names line, column and offset of the first bad byte.
- `std.json.read` and `write` on 10 documents give the same `Value` and text in the evaluator and on both targets (a test in `crates/ignite/tests/`, the differential of the quality ratchet where it exists).
- `DataLimits` bound depth and size in `std.json.read_with`, a test each.
- `docs/layering.md` and `layering::` place `std.data` as a leaf of the base layer.
- `docs/slice.md` gains the section for `std.data`; `task gate` green.

## Scope

- Own: `std/data.ignite` (new), `std/json.ignite`.
- Shared with the later steps: `docs/layering.md`, `crates/ignite-check/tests/layering.rs`, `docs/slice.md`, `AGENTS.md` layout table, `crates/ignite/tests/main.rs`.
