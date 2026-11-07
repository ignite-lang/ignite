---
format: aep.planning-md/3
id: story:parser-error-recovery
kind: story
status: draft
title: Syntax errors recover per item; no empty diagnostic ranges
revision: 1
---
# Story: parser-error-recovery

## Outcome

A file with a syntax error still gets every syntax error of the file and the check diagnostics of every item the parser could read, with non-empty ranges, so an editor shows useful diagnostics while code is being typed.

## Context

- Timo, 2026-11-08, using ignite-lsp in VS Code: "not even diagnostics seem to work well".
- Measured over stdio (ignite-lsp at `f716718`, ignite v0.13.0): appending `fn zz() -> Int = nope + ` to `examples/todo/todo.ignite` gives one diagnostic, `E002 unexpected end of file, expected expression`, with an empty range at 430:0; every other diagnostic of the file is gone while it stands.
- The parser (`crates/ignite-syntax`) stops at the first error.

## Acceptance

- `ignite check --json` on a file with two syntax errors in different items reports both, and reports the type errors of the items that parsed (fixtures with `.expected`).
- No diagnostic has an empty range (`start == end`); an end-of-file error points at the last token.
- `task gate` green.
