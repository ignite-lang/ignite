---
format: aep.planning-md/3
id: story:store-borrowing-reads
kind: story
status: draft
title: Store reads without a second heap copy on LLVM
summary: values, get and update on the LLVM target copy each value at most once
scope:
- confidence: inferred
  path: crates/ignite-io/src/store.rs
- confidence: inferred
  path: crates/ignite-rt/src
revision: 2
---
# Story: store-borrowing-reads

## Outcome

`Store` reads on the LLVM target (`values`, `get`, `update`) copy each value once at most, so a
read of the whole store no longer holds two heap copies of it.

## Context

- Found by the playground session (2026-11-04, ember): a massif run shows FLUSHDB's `values()`
  copying the store twice on LLVM, 10 MB from `store_values` → `copy_to_heap` and 10.7 MB from
  `copy_from_heap`. Its estimate, not measured: −10 MB flush peak, −80k of 185k blocks in a dhat
  run.
- `ignite_io::Store` lives in `crates/ignite-io/src/store.rs`; the LLVM adapter in
  `crates/ignite-rt/src`.

## Acceptance

- The same massif measurement of the ember flush shows one copy of the values at peak, recorded
  before and after in `docs/performance/`.

## Out of Scope

- Store clear and in-place update (story:store-api-gaps).
