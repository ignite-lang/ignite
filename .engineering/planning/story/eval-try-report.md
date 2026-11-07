---
format: aep.planning-md/3
id: story:eval-try-report
kind: story
status: draft
title: Evaluator reports a failed ? as the targets do
revision: 1
---
# Story: eval-try-report

## Outcome

A failed `?` in the evaluator reports the same position and error text as both compiled targets.

## Context

- Found by the differential runner of `story:quality-ratchet` (branch `feat/quality-ratchet`, `crates/ignite/tests/differential.rs`, 2026-11-07): the evaluator reports column 11 and `Err(IoError("not a number: x"))`, both targets `basics.ignite:50:21` and `Err(not a number: x)`.
- Listed as a known difference in `quality/baseline.json` with an ignored failing test for this story.

## Acceptance

- The ignored test passes and is un-ignored; the known difference is removed from the baseline (metric 1.1 improves).
- `task gate` green.
