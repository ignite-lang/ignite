---
format: aep.planning-md/3
id: story:assert-shows-where-texts-differ
kind: story
status: draft
title: A failed assert on long texts shows where they differ
revision: 1
---
# Story: a failed assert on long texts shows where they differ

## Outcome

When `assert a == b` fails on two long or multi-line `Text` values, the report of `ignite test` shows the first place they differ with context, in addition to what it prints today.

## Why

Requested on 2026-11-12 by the author of a command-line tool written in Ignite: its tests compare an encoded document against a raw-string literal, and a failure prints `left:` and `right:` in full (`docs/slice.md` section 12), two long lines to compare by eye. The source line of the test stays as it is; only the report changes.

## Acceptance

- For `==` on two `Text` (and `Bytes`) values of which one is longer than a stated length or holds a line break, the report adds: the byte offset and the line and column of the first difference, and the two values around it with a stated number of characters of context, the differing part marked.
- For multi-line values the report shows the differing lines with their line numbers; lines that are equal are not printed beyond the context.
- `left:` and `right:` stay; whether they are shortened past a stated length is decided in the story and stated in `docs/slice.md`.
- The report is the same on the evaluator and both targets (the runner formats it, `crates/ignite-io/src/report.rs`), and a test holds the exact text for: a difference at offset 0, in the middle, one value a prefix of the other, a difference in the last line.
- No colour codes unless the output is a terminal; the JSON report of the runner, if it has one, carries the offset as a field.
- `docs/slice.md` section 12 states the format; `task gate` green.

## Scope

- `crates/ignite-io/src/report.rs`, `crates/ignite-io/src/test.rs`, `docs/slice.md`. Inferred.

## Not in this story

- A structural diff of records, lists and maps.
