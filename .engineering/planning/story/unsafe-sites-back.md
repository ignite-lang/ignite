---
format: aep.planning-md/3
id: story:unsafe-sites-back
kind: story
status: implemented
title: The substrate holds no more unsafe sites than at v0.15.0
relations:
- depends_on: story:readiness-syscalls-per-connection
scope:
- confidence: cited
  path: crates/ignite-io/src/census.rs
- confidence: cited
  path: crates/ignite-io/src/child.rs
- confidence: cited
  path: crates/ignite-io/src/fs.rs
- confidence: cited
  path: crates/ignite-io/src/net.rs
- confidence: cited
  path: crates/ignite-io/src/reactor.rs
- confidence: cited
  path: crates/ignite-io/src/sched.rs
- confidence: cited
  path: crates/ignite-io/src/signal.rs
- confidence: cited
  path: crates/ignite-io/src/task.rs
- confidence: cited
  path: crates/ignite-io/src/text.rs
- confidence: cited
  path: crates/ignite-rt/src/arena.rs
- confidence: cited
  path: crates/ignite-rt/src/channel.rs
- confidence: cited
  path: crates/ignite-rt/src/fail.rs
- confidence: cited
  path: crates/ignite-rt/src/lib.rs
- confidence: cited
  path: crates/ignite-rt/src/net.rs
- confidence: cited
  path: crates/ignite-rt/src/process.rs
- confidence: cited
  path: crates/ignite-rt/src/store.rs
- confidence: cited
  path: crates/ignite-rt/src/test.rs
- confidence: cited
  path: crates/ignite-rt/src/text.rs
- confidence: inferred
  path: quality/baseline.json
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-11T00:24:53Z", actor: "human:timo", revision: 4, executor: "agent:claude-coordinator", correlation: "wave-11"}
- {from: "proposed", to: "active", at: "2026-11-11T00:24:53Z", actor: "human:timo", revision: 5, executor: "agent:claude-coordinator", correlation: "wave-11"}
- {from: "active", to: "implemented", at: "2026-11-11T23:22:41Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1,"review_outcome":2}}}
---
# Story: the substrate holds no more `unsafe` sites than at v0.15.0

## Outcome

Row 3.6 is at or below its v0.15.0 values: `rust` 12 (now 25), `llvm` 82 (now 95).

## Why

Timo, 2026-11-10, asked whether 3.6 blocks v0.16.0 until it is back at 82 / 12: "A" (it blocks).
The sites came with the readiness scheduler and the leak census.

Diagnosis (`.engineering/waves/2026-11-11-wave-11-diagnosis/counts-unsafe.md`): `ignite-rt` holds the same 70 sites
as at v0.15.0; `ignite-io` went 12 -> 25 (16 new, 3 removed since). Tried in a copy, tests of
both crates green in every state:

- `ignite-io` 25 -> 15: the reactor takes `BorrowedFd<'_>` (-3); `Slot::release`, the test helper
  `fill` and `accept_socket` through safe wrappers the crate already depends on (-3); `fs.rs`
  `entries` over a checked slice (-1); `safe fn malloc_trim` (-1); the three `sigaction` sites in
  one function (-2).
- `ignite-rt` production 70 -> 55 and test modules 55 -> 29.
- `rust` 12 needs three more in `ignite-io` out of: the counting allocator without its `realloc`
  override (-2), a safe byte search instead of `memchr`/`memmem` (-1), `getdents64` through the
  directory wrapper the crate already depends on (-1). Each may cost speed or system calls.

## Acceptance

- `quality::static_metrics_hold_the_bar` measures `rust` <= 12 and `llvm` <= 82, and the bars are
  blessed down to the measured values; the two decisions on 3.6 are withdrawn.
- No site is removed by moving it into a new dependency, a macro or a file the counter does not
  read, and the counter is unchanged.
- No check gets weaker: `Readiness` deregisters its descriptor when dropped (it owns or borrows
  the descriptor for its lifetime; no `debug_assert!` in place of the deregistration); the
  scripted reactor double lends a real descriptor.
- The census stays exact on both targets (the tests of `ignite_io::census` pass unchanged).
- `cargo test -p ignite-io`, `-p ignite-rt`, `-p ignite-llvm` and the `leak::` tests of `ignite`
  pass.
- The removals on a request path (`realloc`, the byte search, `slot_at`, `wrapping_add`) are
  listed in the unit's report as "not measured locally"; the dedicated-host run of the release
  candidate decides whether they stay. A row of 3.3, 3.7 or 3.8 that the run traces to one of
  them comes back to this story.

## Scope

- `crates/ignite-io/src/reactor.rs`, `net.rs`, `fs.rs`, `task.rs`, `sched.rs`, `child.rs`,
  `census.rs`, `text.rs`, a new `signal.rs`. Cited.
- `crates/ignite-rt/src/arena.rs`, `lib.rs`, `net.rs`, `text.rs`, `process.rs`, `test.rs`,
  `fail.rs`, `store.rs`, `channel.rs`. Cited.
- Runs after `story:readiness-syscalls-per-connection` in the same slot: both change
  `crates/ignite-io/src/net.rs`, `sched.rs` and `reactor.rs`.
