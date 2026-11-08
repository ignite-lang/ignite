---
format: aep.planning-md/3
id: story:channel-release-on-close
kind: story
status: proposed
title: A closed channel's memory is freed
summary: 'Case study G30: a task''s closed channel is reported to leak about 370 B'
relations:
- informed_by: story:channels-and-timers
scope:
- confidence: inferred
  path: crates/ignite-io/src/channel.rs
- confidence: inferred
  path: crates/ignite-llvm/src/lower.rs
- confidence: inferred
  path: crates/ignite-llvm/src/own.rs
- confidence: inferred
  path: crates/ignite-rt/src/channel.rs
- confidence: inferred
  path: crates/ignite-rt/src/net.rs
- confidence: inferred
  path: crates/ignite/tests/main.rs
- confidence: inferred
  path: docs/slice.md
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T15:00:48Z", actor: "human:timo", revision: 3}
---
# Story: channel-release-on-close

## Outcome

A channel whose ends are closed and gone frees its memory on both targets: a process that spawns any number of tasks, each creating, closing and polling its own channel, stays at a bounded resident size.

## Context

- Case study gap G30 (`docs/research/case-study-gaps.md`), on v0.15.0: a spawned task running `let (r, b) = Channel.bounded(1); r.close(); b.receive_within(0 ms)` grows the resident size by about 370 B per task (200,000 tasks: 14 MB to 88 MB). The service works around it by keeping closed channels on an idle list and reusing them.
- Where it lives: `crates/ignite-io/src/channel.rs` (`Shared` behind two `Arc<End>`s, freed when both ends' handles are dropped); LLVM runtime `crates/ignite-rt/src/channel.rs` (each end a `Handle::Sender` / `Handle::Receiver` in the handle table of `crates/ignite-rt/src/net.rs`, `register`, `release_owner`, the task's `OWNED` list, `ignite_rt_own_release`); LLVM lowering `crates/ignite-llvm/src/lower.rs` (`own_streams`, `release_owned`, `is_stream`) and `crates/ignite-llvm/src/own.rs`. `docs/slice.md` 30.3 and 30.4 state the intended ownership.
- Not reproduced on `main` (05574a7c8, 2026-11-08): 200,000 spawned tasks running the three lines, one at a time and 1,000 at once, and 200,000 loops of them inside one task, stay at 2 to 6.5 MB peak resident size on both targets, dev, `--release` and `--static`, the same as the control without a channel (`docs/research/case-study-gaps.md` G30). The first step is a reproducer in the service's own shape (what else the task holds, how long it lives, how many run at once, the build flags), obtained from the case study; the cause (ends never released, handle-table slots or segments kept, or growth per task rather than per channel) follows from it.

## Acceptance

- A test in `crates/ignite/tests/` builds a program on each target whose main spawns 200,000 tasks one after another, each running G30's three lines, and reads the program's own resident size (`/proc/self/status` `VmRSS` or the runtime's equivalent) after 20,000 and after 200,000 tasks; the second is at most the first plus 4 MB. The same program without the channel is the control and stays inside the same bound. Measured inside the test, not a load test.
- The program of that test is the case study's reproducer in its own shape; the test fails on `main` before the fix on the target(s) that leak. If no shape leaks on `main`, the story ends with the test on `main` green and G30 recorded as not reproduced.
- `docs/slice.md` 30.3 states when a closed channel's memory is freed; `task gate` green.

## Scope

- `crates/ignite-rt/src/channel.rs`, `crates/ignite-rt/src/net.rs`, `crates/ignite-io/src/channel.rs`, `crates/ignite-llvm/src/lower.rs`, `crates/ignite-llvm/src/own.rs`, `crates/ignite/tests/`, `docs/slice.md`. Inferred.
