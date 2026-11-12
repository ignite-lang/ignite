---
format: aep.planning-md/3
id: story:raw-text-literals-that-interpolate
kind: story
status: draft
title: A raw text literal that interpolates
relations:
- informed_by: story:std-html-escape
revision: 1
---
# Story: a raw text literal that interpolates

## Outcome

A text literal can hold `"` without escapes and still interpolate `{expr}`, so HTML and JSON written inline read as HTML and JSON.

## Why

Requested on 2026-11-12 by the author of a command-line tool written in Ignite that renders pages by concatenation: a raw string (`r"…"`, `r#"…"#`) has "no escapes, no interpolation" (`docs/slice.md`, the literal table), so every line that needs a value is a normal string with each quote escaped. Row 24 of `docs/friction.md` is the neighbouring case (literal braces beside an interpolation).

## Acceptance

- One literal form, decided in the story (for example `$r#"…{e}…"#`), in which `"` and `\` are literal and `{expr}` interpolates as in a normal string; `{{` and `}}` are literal braces as there.
- The number of `#` closes it as for `r#"…"#`; line breaks are kept.
- It is a desugar to the concatenation a normal string gives: the checker, the evaluator and both targets see no new node kind, and a differential test holds one program on all three.
- The form meets the five proposal rules of `docs/syntax.md` section 1.2, stated there with the reserved-word count unchanged.
- The lexer refuses an unclosed literal and an unclosed `{` with the position of the opening.
- The grammar and the literal table of `docs/slice.md`, `site/book`, and row 24 of `docs/friction.md` (solved, or its remainder stated); `task gate` green.

## Scope

- The lexer and parser crates, `docs/slice.md`, `docs/syntax.md`, `docs/friction.md`, the editor grammar if the repository holds one. Inferred.

## Not in this story

- Multi-line literals with stripped indentation (`story:multi-line-text-literals`).
- HTML escaping of the interpolated value (`story:std-html-escape`); views (`story:markup-views`).
