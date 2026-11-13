---
format: aep.planning-md/3
id: story:path-cost-cells-above-v0-16-3
kind: story
status: draft
title: The path-cost cells that v0.16.4 raised are back at v0.16.3
relations:
- informed_by: story:path-cost-counted-at-every-merge
- informed_by: story:rust-target-text-reads-borrow
revision: 1
---
# Story: the path-cost cells that v0.16.4 raised are back at v0.16.3

## Outcome

The four cells of row 3.10 of `docs/quality.md` that v0.16.4 recorded above v0.16.3 beyond their bands are at their v0.16.3 values or below, or the story states for a cell why its v0.16.4 value stays.

## Why

`task path-cost` on the release commit of v0.16.4 (`3f221dfb`) against the counts of v0.16.3 refused four cells, each measured three times. All four are the compiler's own cost on the scenario `design`; no cell of a served request is among them.

- `all/design/check` instructions: 851,984,229 at v0.16.3, 853,086,407 at v0.16.4 (+0.129 %, band 851,984; the three runs 853,136,735, 853,086,407, 853,133,398).
- `all/design/plan` instructions: 857,679,569 at v0.16.3, 858,792,916 at v0.16.4 (+0.130 %, band 857,679; the three runs 858,903,357, 858,792,916, 858,829,892).
- `llvm/design/build` instructions: 2,227,556,478 at v0.16.3, 2,235,874,272 at v0.16.4 (+0.373 %, band 2,227,556; the three runs 2,236,324,735, 2,235,988,732, 2,235,874,272).
- `rust/design/build` instructions: 1,163,103,416 at v0.16.3, 1,165,359,006 at v0.16.4 (+0.194 %, band 1,163,103; the three runs 1,165,359,006, 1,165,388,773, 1,165,436,521).

The functions the check names as grown, in all four cells: `ignite_types::moves::Walk::call` 3,625,459 to 3,753,048 and its recursion 1,431,272 to 1,489,500, `ignite_types::moves::walk` 202,239 to 233,503, `ignite_types::moves::analyse` 157,158 to 170,894, and under them `memcpy`, the lookups of a `hashbrown` table keyed by `NodeId` and `Vec<moves::Node>` push and pop. The release holds one change of that analysis: the first unit of `story:rust-target-text-reads-borrow` (a text or byte string that is only read is lent; a buffer is taken at its last use), which records per function which buffers are read and where they are last used. That unit's own count before the release read `ignite check` +0.151 % on the design file, the same size as the two cells `check` and `plan`.

In `llvm/design/build` the larger part is the linker: `liblldELF.so` 112,649,976 to 114,843,147, `InputSection::writeTo` 32,328,054 to 33,293,640, `libLLVM.so` 102,714,519 to 103,421,074, `libz` 23,083,566 to 23,508,541. Inferred, not checked: the object or the runtime archive the driver links is larger at v0.16.4 (row 3.2 reads `llvm/todod/dev` 7,975,704 to 7,976,544 bytes), and the linker's work follows the bytes. In `rust/design/build` the check also names `ignite_rustgen::Generator::block` (under 116,310 to 136,253), a `HashMap<NodeId, ()>::contains_key` (under 116,310 to 120,459) and `malloc` (`_int_malloc` 24,181,114 to 24,335,285). Inferred, not checked: the generator asks, per block, which bindings the new analysis marked.

The release was cut with these cells tracked and not refused (the rule of 2026-11-11), and its `RECORD` wrote the v0.16.4 values as the values a merge is held to.

Of the other 32 cells none was above its band. Below v0.16.3 by more than the band: `rust/todo/patch` 798,633 to 789,919 and `rust/todo/delete` 532,880 to 530,865, the request cells that read texts the release no longer copies.

The three cells of `story:path-cost-cells-above-v0-16-2` are a separate account against v0.16.2; `rust/design/build` is in both.

## Acceptance

- `all/design/check`, `all/design/plan`, `llvm/design/build` and `rust/design/build` read at most 851,984,229, 857,679,569, 2,227,556,478 and 1,163,103,416 instructions by `task path-cost`, or the story records for a cell the cause and why the value stays, with the count of what the rise buys.
- The allocations the first unit of the text reads removed stay removed (`crates/ignite/tests/text_reads.rs` and `crates/ignite/tests/text_reads_adversary.rs` unchanged).
- `task gate` green.

## Scope

- `crates/ignite-types/src/moves.rs` (`Walk::call`, `walk`, `analyse`: what the analysis of the read-only texts records per call and per node), `crates/ignite-rustgen/src/lib.rs` (`Generator::block`, the lookups of the marked bindings). Inferred.
- The linker part of `llvm/design/build` belongs to `story:binary-size-back` when the cause is the size of the object.
