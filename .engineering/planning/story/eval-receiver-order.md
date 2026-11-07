---
format: aep.planning-md/3
id: story:eval-receiver-order
kind: story
status: draft
title: Evaluator evaluates a builtin's receiver first
revision: 1
---
# Story: eval-receiver-order

## Outcome

The evaluator evaluates a builtin's receiver before its arguments, as both compiled targets do.

## Context

- Found by the differential runner of `story:quality-ratchet` (2026-11-07): in two `std/text.ignite` tests the evaluator reads the receiver after the arguments and gives 99 where both targets give 23.
- Listed as a known difference in `quality/baseline.json` with an ignored failing test for this story. `story:mid-level-ir` would make the order one rule for all three consumers.

## Acceptance

- The ignored test passes and is un-ignored; the known difference is removed from the baseline.
- `task gate` green.
