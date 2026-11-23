---
format: aep.planning-md/3
id: story:import-order-linear-in-independent-modules
kind: story
status: draft
title: Ordering independent modules does not rescan the ones already ordered
revision: 1
---
# Story: ordering independent modules does not rescan the ones already ordered

## Outcome

`import_order` gives the same order as today (smallest ready path first) and the same E032 for a cycle, with work linear in modules and imports plus the cost of a sorted ready set.

## Why

IG-33, severity medium, class performance; an operation count derived from source (the maintainer review of 2026-11-23 against main at a1d1407e). No wall-clock time was measured.

`import_order` (`crates/ignite/src/lib.rs:854` to `:882`) finds each next ready module by scanning the sorted edge map from its start (`:868` to `:872`). For N independent modules that is 1 + 2 + ... + N entries. It orders the standard library (`:808`) and the sibling modules (`:1236`) on every compiler command.

## Acceptance

- A counting test, red first: the graph visits for N disjoint modules and for an N-long chain are linear in N.
- Controls: the order of the standard library and of the example programs is unchanged; a cycle gives the same E032; `task gate` green.

## Scope

- `crates/ignite/src/lib.rs:854`, `:868`, `:1236` (cited). Each read at a1d1407e, as the review states.
