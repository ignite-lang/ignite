---
format: aep.planning-md/3
id: story:llvm-lowering-copies
kind: story
status: draft
title: LLVM lowering without avoidable copies
revision: 1
---
# Story: llvm-lowering-copies

## Outcome

The LLVM lowering and runtime stop making the avoidable copies and heap allocations found by the
review of 2026-11-05, each step measured.

## Context

Found by the LLVM review of 2026-11-05 (origin/main `39e08768`), inferred from code:

| id | file:line | finding |
|---|---|---|
| P1 | `crates/ignite-llvm/src/lower.rs:2745-2751, 3637-3645`, `crates/ignite-rt/src/text.rs:44-52` | `Text +` and `concat`: runtime call, heap `Vec`, two copies; a chain copies the prefix per step |
| P2 | `lower.rs:2090-2095, 2102-2110` | a constant is re-evaluated at every mention (also on the Rust target) |
| P3 | `lower.rs:4131-4132, 2256-2262, 665-672` | `list.map` deep-copies each element; fn-value calls copy aggregate arguments |
| P4 | `lower.rs:2016-2022` | closure captures copied, never moved at last use |
| P5 | `lower.rs:4296-4299, 4341-4345` | a borrowed aggregate passed to `spawn` is copied twice |
| P6 | `crates/ignite-rt/src/net.rs:287-315`, `arena.rs:164-178, 235-240, 397-403` | a read over 16 KiB (HTTP 64 KiB) is an untrimmed `calloc` block and empties the bump state |
| P7 | `rt/text.rs:35-42, 60-87, 100-107`, `rt/stdio.rs:38-44` | join, trim, debug text and stdout writes allocate on the heap first |
| P8 | `lower.rs:1685-1724`, `crates/ignite-llvm/src/glue.rs:369-372` | `Map.put` O(n) memmove (O11); Text key compare is a runtime call |
| P9 | `glue.rs:55` | `icmp ult` sends an exact-fit allocation to the slow path (`ule`) |

Also from the review: `glue.rs:373-377` compares enum keys by tag only, which E106 makes dead
code today; relaxing E106 would miscompile.

## Acceptance

- Per step: the call or copy count in the program `.ll` named in the review drops, the inline
  tests pass on both targets, a before/after timing of the affected program is recorded under
  `docs/performance/`, and `task gate` is green.

## Out of Scope

- M1 and L1 of the same review (`story:llvm-stream-ownership`).
