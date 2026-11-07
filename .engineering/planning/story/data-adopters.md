---
format: aep.planning-md/3
id: story:data-adopters
kind: story
status: draft
title: Replace the todo codecs; measure ignite-lsp against the derived codec
relations:
- decomposes: epic:data-formats
- depends_on: story:data-derived-codec
revision: 1
---
# Story: data-adopters

## Outcome

The todo example's hand-written codecs are replaced by `encode`/`decode`, and the ignite-lsp generated codecs are measured against `Shape { keys = Camel }` so the next step for ignite-lsp is a number, not a guess.

## Acceptance

- `examples/todo/todo.ignite` has no `to_json`/`to_record`/hand decoder; its files and HTTP bodies are byte-equal to before (the existing todo suites stay green unchanged).
- A report in `docs/research/` lists, for the LSP types in `ignite-lsp/src/lsp_types.ignite`, how many decode to the same `Json` through the derived codec with `Case.Camel` plus renames, and which do not and why.
- `task gate` green.

## Scope

- `examples/todo/todo.ignite`, `docs/research/` (new report). Inferred.
