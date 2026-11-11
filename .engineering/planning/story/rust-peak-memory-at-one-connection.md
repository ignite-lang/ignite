---
format: aep.planning-md/3
id: story:rust-peak-memory-at-one-connection
kind: story
status: implemented
title: A Rust-target server with one connection peaks at no more memory than at v0.15.0
relations:
- informed_by: story:binary-size-back
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-11T18:03:55Z", actor: "human:timo", revision: 2, executor: "agent:claude-coordinator", correlation: "wave-11"}
- {from: "proposed", to: "active", at: "2026-11-11T18:03:55Z", actor: "human:timo", revision: 3, executor: "agent:claude-coordinator", correlation: "wave-11"}
- {from: "active", to: "implemented", at: "2026-11-11T23:22:42Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}}
---
# Story: a Rust-target server with one connection peaks at no more memory than at v0.15.0

## Outcome

Row 3.8 of `docs/quality.md` for `rust/terminal/rr-1` and `rust/todo/get-1` is at or below v0.15.0 on the dedicated host.

## Why

The dedicated-host run of 2026-11-11 (`iq-20261111t171623z-cf0f`, v0.15.0 against the integration head with unit size, 5 passes interleaved) has both cells worse in every pass:

- `rust/terminal/rr-1`: 1.069× at v0.15.0 (1058 to 1074), 1.137× at the head (1126 to 1172), ratio 1.064.
- `rust/todo/get-1`: 0.781× at v0.15.0 (758 to 785), 0.814× at the head (789 to 817), ratio 1.042.

The LLVM cells of the same row are lower than at v0.15.0 (0.904 and 0.909). The Rust binaries of rows 3.2 and 3.9 are above v0.15.0 by a decision; whether the resident pages of the larger binary, the worker and task stacks, or the allocator's arenas make the difference is not measured.

## Acceptance

- The peak resident set of the Rust `terminald` and `todod` with one connection is itemized by mapping (`/proc/<pid>/smaps` at the peak) at v0.15.0 and at the head, and each difference is named with the code that makes it.
- Both cells are not worse than v0.15.0 in the next dedicated-host run; no other cell of rows 3.3, 3.7 and 3.8 is worse than in the run named above.
- No program starts, serves or builds with more instructions for it (the counts of unit size: start, request, connection, session, both targets).
- `task gate` green; no row of `docs/quality.md` worse.

## Scope

- `crates/ignite-io/src/`, `crates/ignite-rustgen/src/lib.rs` (profiles of the generated project). Inferred.
