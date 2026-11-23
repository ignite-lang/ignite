---
format: aep.planning-md/3
id: story:typed-json-columns-without-rescanning-the-line
kind: story
status: draft
title: Typed JSON computes columns without rescanning the line
relations:
- informed_by: story:typed-program-json
revision: 1
---
# Story: typed JSON computes columns without rescanning the line

## Outcome

`ignite emit --typed-json` converts span offsets to columns with work linear in the line length overall; the columns (in characters, multibyte text included) are unchanged.

## Why

IG-35, severity medium, class performance; an operation count derived from source (the maintainer review of 2026-11-23 against main at a1d1407e). No wall-clock time was measured.

`span` converts the start and end of every node (`crates/ignite/src/typed_json.rs:308` to `:311`). The line starts are cached, but `line_col_in` counts the characters from the start of the line on each call (`crates/ignite-syntax/src/diagnostic.rs:88` to `:97`, the count at `:95`; its own comment at `:82` says linear in the line). On a line with N fixed-width items the scanned prefixes add up to quadratic work.

## Acceptance

- A counting test, red first: characters scanned for a line of N items is linear in N.
- Controls: a golden test with multibyte text before the spans keeps its columns; the existing typed-JSON goldens unchanged; `task gate` green.

## Scope

- `crates/ignite/src/typed_json.rs:308`, `crates/ignite-syntax/src/diagnostic.rs:88` (cited). Each read at a1d1407e, as the review states.
