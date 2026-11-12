---
format: aep.planning-md/3
id: story:std-text-cutting
kind: story
status: draft
title: std.text cuts a text without a helper module
revision: 1
---
# Story: std.text cuts a text without a helper module

## Outcome

`std.text` has the cutting functions a line-oriented parser writes first: the last occurrence, the parts before and after a separator, a prefix and suffix that cannot fail, words, and ASCII class tests.

## Why

Requested on 2026-11-12 by the author of a command-line tool written in Ignite: it carries a utility module of text helpers, some of them written twice under different names. `std/text.ignite` has `find`, `find_from`, `contains`, `ends_with`, `strip_suffix`, `split`, `split_once`, `replace`, `trim_start`, `trim_end`, `lines`, `slice`, `is_whitespace` and `is_digit`. Its `slice` fails out of range, so a clamped prefix is written by hand. By frequency in that program the helpers rank: clamped prefix and suffix, before and after a separator, strip one pair of quotes, last occurrence, leading spaces, words, total prefix and suffix removal, digit-only test.

## Acceptance

- `find_last(text, part) -> Option<Int>`: the byte offset of the last occurrence.
- `before(text, sep)` and `after(text, sep)`: the part before and after the first `sep`; without `sep`, `before` is the whole text and `after` is empty. `after_last` and `before_last` for the last `sep`.
- `take(text, n)` and `drop(text, n)`: the first `n` bytes and the rest, clamped to the length; a cut inside a UTF-8 sequence is stated in the doc comment and pinned by a test.
- `without_prefix(text, p)` and `without_suffix(text, s)`: the text without it, or unchanged.
- `words(text) -> List<Text>`: split on runs of whitespace, no empty elements.
- `is_alpha(b)`, `is_alphanumeric(b)` on a `Byte`, and `all_digits(text)`.
- The names are decided in the story against the ones `std.text` has; every function has inline tests with the empty text, a text without the separator and a separator at both ends.
- Allocations per call on the Rust target are counted for `before`, `take` and `words` and written into the doc comments' section of `docs/slice.md`.
- `docs/slice.md` lists them; `task gate` green.

## Scope

- `std/text.ignite`, `docs/slice.md`. Inferred.

## Not in this story

- Regular expressions (`story:std-regex`), Unicode classes and case mapping beyond what `std.text` has.
- Quote stripping and indentation counting: one line each over the functions above.
