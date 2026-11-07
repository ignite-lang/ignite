---
format: aep.planning-md/3
id: story:parser-nested-if-stack
kind: story
status: draft
title: 100 nested if-expressions parse on a 2 MiB thread
tags:
- from-finding
relations:
- informed_by: review-result:adversary-node-ids-pass-1
revision: 1
---
# Story: parser-nested-if-stack

## Outcome

100 nested `if` expressions parse on a 2 MiB thread, as 100 nested parentheses do.

## Context

- Found by the adversary of `story:stable-node-ids` (`review-result:adversary-node-ids-pass-1`, 2026-11-07): `node_ids_adversary::stack_100_ifs` overflows a 2 MiB thread and aborts the test binary on the base `64585854` and on `a2e04f97`; `records` overflowed on the base and passes after the parser split of that story.
- Site: `crates/ignite-syntax/src/parser.rs:2567` (`primary`, at `a2e04f97`).
- What reaches it: no user; the CLI parses on a larger stack (`crates/ignite/src/main.rs:479`). Test threads and any library caller on a default thread do.

## Acceptance

- A test parses 100 nested `if` expressions on a 2 MiB thread and passes; the other nesting forms of `node_ids_adversary::stack_100_*` still pass.
