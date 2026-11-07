---
format: aep.planning-md/3
id: epic:data-formats
kind: epic
status: draft
title: 'Data formats: one value model, derived codecs, JSON, YAML, TOML'
revision: 1
---
# Epic: data-formats

## Outcome

An Ignite program turns any data value into JSON, YAML or TOML text and back with `encode`/`decode` and a format's `parse`/`emit`, with no encoder or decoder written per type, on both targets and in the evaluator.

## Context

- Timo, 2026-11-08: "create design for serde (yaml, json, toml, etc)".
- Design: `docs/serde.md` (PR 98, `fa292edf`).
- Hand-written codecs today: `examples/todo/todo.ignite:59-70`, `std/protocol/jsonrpc.ignite:250-371`, ignite-lsp `src/lsp_types.ignite` (9,882 lines, 533 codec functions, generated).
- Pusher case study G9 needs YAML (`docs/friction.md` row 68).

## Stories

| step | story | needs |
|---|---|---|
| 1 | `data-value-model` | |
| 2 | `data-derived-codec` | 1 |
| 3 | `data-yaml` | 1 |
| 4 | `data-toml` | 1 |
| 5 | `data-adopters` | 2 |

## Not covered

Streaming, binary formats, CSV, hand-written per-type codecs (need user `interface`/`impl`), typed configuration at check time (needs declared build inputs): `docs/serde.md` sections 7 and 9.
