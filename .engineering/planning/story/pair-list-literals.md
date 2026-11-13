---
format: aep.planning-md/3
id: story:pair-list-literals
kind: story
status: draft
title: Pair-list literals
revision: 1
---
# Story: pair-list literals `["key": value]`

## Outcome

`[k: v, …]` is the list of pairs `[(k, v), …]`, so a JSON object and a header list are written without a tuple around every member.

## Why

`docs/syntax.md` holds it as proposal P11, rank 9 of its priority table (section 6), unscheduled. Requested on 2026-11-12 by the author of a command-line tool written in Ignite for its JSON objects; it saves tokens there and no lines, which is why it ranks low in that report too.

## Acceptance

- `[k: v, …]` has the type `List<(K, V)>`; mixing pair and plain elements is E001 (`docs/syntax.md` P11).
- It is a desugar to the list of tuples; a differential test holds one program on the evaluator and both targets.
- What the literal means where a `Map<K, V>` is expected is decided and stated: a list only, or also a `Map`; `docs/syntax.md` says the literal is reserved for the `Map` case.
- The rewrites of `docs/syntax.md` section 9 that use it are applied to `std/json.ignite`'s tests and `examples/` where they exist.
- `docs/slice.md` grammar and section, `site/book`; `task gate` green.

## Scope

- The parser and checker crates, `docs/slice.md`, `docs/syntax.md`. Inferred.

## Not in this story

- A `Map` literal with its own syntax; `Map.from(pairs)`.
