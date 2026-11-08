---
format: aep.planning-md/3
id: epic:data-formats
kind: epic
status: draft
title: 'Data formats: one value model, derived codecs, JSON, YAML, TOML'
revision: 3
---
# Epic: data-formats

## Outcome

An Ignite program turns a data value into JSON, YAML or TOML text and back with `std.data` codecs and a format's `read`/`write`, all in `std` and written in Ignite, on both targets and in the evaluator. TOML holds only documents whose root is a map and that contain no null (TOML has none); the codec leaves `None` fields out, so a record with optional fields round-trips.

## Context

- Timo, 2026-11-08: "create design for serde (yaml, json, toml, etc)", then "… in std".
- Design: `docs/serde.md` (PRs 98, 99, 100; `main` at `2563ea25`).
- Hand-written codecs today: `examples/todo/todo.ignite:59-70`, `std/protocol/jsonrpc.ignite:250-371`, ignite-lsp `src/lsp_types.ignite` (9,882 lines, 533 codec functions, generated).
- case-study case study G9 needs YAML (`docs/friction.md` row 68).

## Stories

| step | story | needs |
|---|---|---|
| 1 | `data-value-model` | |
| 2 | `data-codec-combinators` | 1 |
| 3 | `data-yaml` | 1 |
| 4 | `data-toml` | 1 |
| 5 | `data-derived-codec` | 2 |
| 6 | `data-adopters` | 2, 3, 4, 5 |

Steps 2, 3 and 4 share `docs/slice.md`, the `AGENTS.md` layout table, `docs/layering.md`, `crates/ignite-check/tests/layering.rs` and `crates/ignite/tests/main.rs`; each names them in its Scope, and a wave that runs them together merges those files on its integration branch.

## Acceptance

- Every story implemented.
- One typed record with an optional field, a list, a map and a payload enum round-trips through JSON, YAML and TOML with a derived codec on the evaluator, Rust and LLVM (`story:data-adopters`).
- Each format module reads the same documents to the same `Value` on the evaluator and both targets (`story:data-yaml`, `story:data-toml`, `story:data-value-model` for JSON).

## Not covered

- Streaming, binary formats, CSV, codecs through `impl`, conformance catalogues and `conforms` for the formats: `docs/serde.md` section 9.
- Typed configuration at check time (needs declared build inputs): section 7.
