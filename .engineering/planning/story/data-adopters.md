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
revision: 2
---
# Story: data-adopters

## Outcome

The todo example uses derived codecs instead of its hand-written ones, one typed record round-trips through all three formats, and the ignite-lsp generated codecs are measured against the derived codec, so the next step for ignite-lsp is a number.

## Acceptance

- `examples/todo/todo.ignite`: `to_json` becomes `encode(codec(), TodoView { id, title, done })` with a new record `TodoView`, and `to_record` becomes `encode(codec(), todo)` over `Todo`'s fields; the hand decoder becomes `decode`. The inline tests that call `to_json` and `to_record` (from `examples/todo/todo.ignite:355`) are rewritten to call the new functions and expect the same text as before. The end-to-end todo suites under `crates/ignite/tests/` pass with no change to their files, so HTTP bodies and data files are byte-equal.
- A fixture record with an optional field, a list, a `Map<Text, _>` and a payload enum round-trips through `std.json`, `std.yaml` and `std.toml` with `codec()` on the evaluator, Rust and LLVM.
- A report `docs/research/lsp-derived-codec.md` counts, for the types in `src/lsp_types.ignite` of the ignite-lsp repository at tag `v0.2.0` (`9b31d37`), how many encode to the same `Json` through `codec()` with `Shape { keys = Camel }` plus renames, and lists the others with the reason.
- `task gate` green.

## Scope

- Own: `examples/todo/todo.ignite`, a fixture and test under `crates/ignite/tests/`, `docs/research/lsp-derived-codec.md` (new).
- Reads, not writes: the ignite-lsp repository at `v0.2.0`.
- Shared: `crates/ignite/tests/main.rs` (one `mod` line).
