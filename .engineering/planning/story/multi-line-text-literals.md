---
format: aep.planning-md/3
id: story:multi-line-text-literals
kind: story
status: draft
title: A multi-line text literal with its indentation stripped
relations:
- informed_by: story:raw-text-literals-that-interpolate
revision: 1
---
# Story: a multi-line text literal with its indentation stripped

## Outcome

A text of several lines is written at the indentation of the code around it, and the literal's value does not hold that indentation.

## Why

Requested on 2026-11-12 by the author of a command-line tool written in Ignite: `docs/slice.md` says "No multi-line strings". A raw string keeps line breaks and also keeps the indentation, so its test fixtures start at column 0 inside indented code, and short fixtures are one line with `\n` between the parts.

## Acceptance

- One literal form, decided in the story against the two candidates (a `"""` block whose common leading indentation is removed, as in Swift, Kotlin and Java; or line strings with a prefix on every line, as in Zig), with the reason in `docs/syntax.md`.
- The rule for what is stripped is one sentence in `docs/slice.md` and is pinned by tests: a blank line inside, a line indented less than the closing delimiter (a diagnostic), a trailing line break present or absent by an explicit rule, tabs.
- Interpolation and escapes behave as in a normal string; whether a raw variant exists is decided with `story:raw-text-literals-that-interpolate`.
- It is a desugar to one text literal: the checker, the evaluator and both targets see no new node kind; a differential test holds one program on all three.
- `ignite fmt` (`story:ignite-fmt`), once it exists, does not change the literal's value.
- The sentence "No multi-line strings" of `docs/slice.md` is replaced; `site/book`; `task gate` green.

## Scope

- The lexer and parser crates, `docs/slice.md`, `docs/syntax.md`. Inferred.

## Not in this story

- Byte-string variants beyond what follows from the lexer rule.
