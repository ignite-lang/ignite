---
format: aep.planning-md/3
id: story:planner-reads-a-component-field-inside-a-helper
kind: story
status: draft
title: The planner reads a component's field inside a pure helper
revision: 1
---
# Story: the planner reads a component's field inside a pure helper

## Outcome

A component passed to a pure helper such as `fn read_score(n: Node) -> Int = n.score` yields its field value in `ignite plan`, as reading `n.score` directly does.

## Why

IG-27, severity medium, class logic; observed with a released command (the maintainer review of 2026-11-23 against main at a1d1407e, v0.16.4 archive).

A component with `score = 7` supplied to the helper failed planning with E117; reading the field directly gave 7.

The evaluator enters a fn with `system: None` in its context (`crates/ignite-eval/src/eval.rs:527` to `:530`). Field access on a `Value::Member` looks the member up through `member_of(cx, ..)` (`:1796` to `:1801`), which needs `cx.system` (`:2436` to `:2445`), although the value carries its declaring system (`crates/ignite-eval/src/value.rs:64`). `docs/slice.md:2381` to `:2383`: a bare component name in a property is a value of its declared type.

## Acceptance

- A plan golden test, red first: the helper form plans 7.
- Controls: the direct read; a helper in another module; a component of a second system passed to a helper reads its own field; `task gate` green.

## Scope

- `crates/ignite-eval/src/eval.rs:529`, `:1796` (cited), `:2436` (`member_of`), `docs/slice.md:2381` (cited). Each read at a1d1407e, as the review states.
