---
format: aep.planning-md/3
id: story:range-exhaustiveness-compares-bounds-not-text
kind: story
status: draft
title: Numeric range exhaustiveness compares bounds, not rendered text
revision: 1
---
# Story: numeric range exhaustiveness compares bounds, not rendered text

## Outcome

The exhaustiveness check of a numeric match passes interval bounds through matching directly and renders them only for diagnostics, so its work grows with arms times intervals, not with arms times intervals squared.

## Why

IG-30, severity medium, class performance; an operation count derived from source (the maintainer review of 2026-11-23 against main at a1d1407e). No wall-clock time was measured.

`Split::cases` renders each interval as its case name (`crates/ignite-types/src/exhaustive.rs:95` to `:105`). For a range pattern, `Split::args` finds the interval again by rendering earlier intervals and comparing text (`:110` to `:121`). The usefulness loop calls it for each case and each row (`:265` to `:272`). With A range arms and B distinct intervals that is A x B(B+1)/2 formatted comparisons in an all-range matrix, and B can grow with A.

## Acceptance

- A counting test, red first: the number of interval lookups for A range arms is at most A x B.
- Controls: the missing-case witnesses (`lo..=hi`, a single value) are unchanged; `task gate` green.

## Scope

- `crates/ignite-types/src/exhaustive.rs:95`, `:110`, `:265` (cited). Each read at a1d1407e, as the review states.
