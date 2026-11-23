---
format: aep.planning-md/3
id: story:conditional-property-keeps-its-uses-edge
kind: story
status: implemented
title: A member name read inside an if, a block or a match of a property gives its uses edge
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T03:51:06Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-23T03:51:06Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-11-23T21:50:09Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
# Story: a member name read inside an if, a block or a match of a property gives its uses edge

## Outcome

The model's `uses` edges list every bare member name read in a property expression, whatever expression encloses it (`if`, block, `match` and every other expression with children), so a property computed through a condition shows the same edge as the direct read.

## Why

IG-10, severity high, class logic; observed with a released command (the maintainer review of 2026-11-23 against main at a1d1407e, v0.16.4 archive).

Synthetic reproducer: two properties read the same component field, one directly and one inside the true branch of an `if`. Both planned values were 7; only the direct property produced the `uses` edge `docs/slice.md:2917` documents.

`uses` (`crates/ignite-eval/src/model.rs:982` to `:1037`) visits names, records, record updates, braces, lists, tuples, calls, methods, named arguments, fields, unary and binary expressions; `if`, blocks, `match` and the rest fall into `_ => {}` (`:1035`). The edges from values (`value_edges`, `:1060`) cannot recover the dependency once the value is an integer.

## Acceptance

- A plan golden test, red first: the edge exists for the direct spelling and for the `if` spelling, and for a read inside a `match` arm and a block.
- Controls: a qualified `System.name` stays a `references` edge, not `uses`; the plans of the examples are unchanged except for added edges.
- `task gate` green.

## Scope

- `crates/ignite-eval/src/model.rs:982` to `:1037` (`uses`, cited `:982`), `:1056` to `:1060` (cited `:1060`), `docs/slice.md:2917`. Each read at a1d1407e, as the review states.
