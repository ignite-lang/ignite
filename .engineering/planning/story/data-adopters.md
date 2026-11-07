---
format: aep.planning-md/3
id: story:data-adopters
kind: story
status: draft
title: Replace the todo codecs; measure ignite-lsp against the derived codec
relations:
- decomposes: epic:data-formats
- depends_on: story:data-derived-codec
- depends_on: story:data-yaml
- depends_on: story:data-toml
- depends_on: story:data-codec-combinators
scope:
- confidence: inferred
  path: crates/ignite/tests/main.rs
- confidence: inferred
  path: docs/research/lsp-derived-codec.md
- confidence: inferred
  path: examples/todo/todo.ignite
revision: 4
---
# Story: data-adopters

## Outcome

The todo example uses derived codecs instead of its hand-written ones, one typed record round-trips through all three formats, and the ignite-lsp generated codecs are measured against the derived codec, so the next step for ignite-lsp is a number.

## Acceptance

- `examples/todo/todo.ignite`: `to_json` becomes `encode(codec(), TodoView { id, title, done })` and `to_record` becomes `encode(codec(), TodoFile { id, title, done, version, deleted })`, two new records with the fields each writes today (`Todo.deleted_by` is in neither: memory only); `from_record` becomes `decode` into `TodoFile`. The inline tests that call `to_json` and `to_record` (from `examples/todo/todo.ignite:355`) are rewritten to call the new functions and expect the same text as before. A new inline test writes one todo through the new `to_record` and compares the text with the literal the old `to_record` gave for the same todo, so the data file bytes are checked. The end-to-end todo suites under `crates/ignite/tests/` pass with no change to their files.
- A fixture record with an optional field, a list, a `Map<Text, _>` and a payload enum round-trips through `std.json`, `std.yaml` and `std.toml` with `codec()` on the evaluator, Rust and LLVM.
- A report `docs/research/lsp-derived-codec.md` counts, for the types in `src/lsp_types.ignite` of the ignite-lsp repository at tag `v0.2.0` (`9b31d37`), how many encode to the same `Json` through `codec()` with `Shape { keys = Camel }` plus renames, and lists the others with the reason.
- `task gate` green.

## Scope

- Own: `examples/todo/todo.ignite`, a fixture and test under `crates/ignite/tests/`, `docs/research/lsp-derived-codec.md` (new).
- Reads, not writes: the ignite-lsp repository at `v0.2.0`.
- Shared: `crates/ignite/tests/main.rs` (one `mod` line).
