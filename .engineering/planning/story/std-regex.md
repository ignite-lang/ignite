---
format: aep.planning-md/3
id: story:std-regex
kind: story
status: draft
title: 'std.regex: regular expressions that match in linear time'
revision: 1
---
# Story: std.regex, regular expressions that match in linear time

## Outcome

`std.regex` compiles a pattern once and matches it against a `Text` or `Bytes` in time linear in the input: `Regex.compile(pattern) -> Result<Regex, RegexError>`, `is_match`, `find`, `find_all`, `captures` (numbered and named groups) and `replace_all`, written in Ignite, on the evaluator and both targets.

## Why

Requested on 2026-11-12 by the author of a command-line tool written in Ignite: at v0.16.2 `std.text` has substring search (`find`, `contains`), `split`, `split_once` and `replace`, and no patterns; `std` has no regex module.

## Acceptance

- Syntax: literals, `.`, classes (`[a-z]`, `[^..]`, `\d`, `\w`, `\s` and their negations, ASCII), anchors `^` `$` `\b`, groups (capturing, non-capturing, named), alternation, and the quantifiers `*`, `+`, `?`, `{n}`, `{n,}`, `{n,m}`, each greedy and lazy. Backreferences and look-around are refused by `compile` with an error that names the construct and its position.
- Matching never backtracks: a test with `(a*)*b` against 10,000 `a` bounds the instruction count (callgrind) to a constant times the count for 1,000.
- Limits: a compiled program has a stated maximum size; a pattern over it is refused by `compile`, not at match time.
- Leftmost match with the precedence rules stated in `docs/slice.md`; a table of at least 200 pattern/input/expected rows runs on the evaluator and both targets with identical results.
- Unicode: the input is matched as bytes of UTF-8; `.` matches one code point; what a class matches outside ASCII is stated and tested.
- `docs/layering.md` lists the module; `site/book` has a page; `task gate` green.

## Scope

- `std/regex.ignite` (new), `docs/layering.md`, `crates/ignite-check/tests/layering.rs`, `docs/slice.md`, `site/book`. Inferred.

## Not in this story

- A regex literal in the language or patterns checked at compile time.
