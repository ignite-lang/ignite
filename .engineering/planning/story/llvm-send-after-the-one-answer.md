---
format: aep.planning-md/3
id: story:llvm-send-after-the-one-answer
kind: story
status: draft
title: On LLVM a send to a channel whose receiver took its one answer is explained
relations:
- informed_by: story:llvm-http-connection-closes-its-socket
revision: 1
---
# Story: on LLVM a send to a channel whose receiver took its one answer is explained

## Outcome

It is known whether a `send` that succeeds on the LLVM target after the receiver has taken its one answer is a defect of the channel or only the order of two wake-ups, and a test holds the answer.

## Why

Observed on 2026-11-11 by the implementor of `story:llvm-http-connection-closes-its-socket` (wave 11, unit fdleak) while writing its descriptor tests (`crates/ignite/tests/stream_ownership.rs`): a second `send` on a channel whose receiver takes one answer succeeded on LLVM in 5 of about 200 runs and on Rust in none of as many. The unit's test was rewritten so that it no longer depends on it. Not explained; the implementor's reading "wake-up latency" is a guess. Pre-existing: the unit changed no channel code.

## Acceptance

- A test of `crates/ignite/tests` reproduces the second successful `send` on LLVM at a stated rate (runs and hits), or 2,000 runs show none and the story is closed as not reproduced with that number.
- If it reproduces: the cause is named with `file:line`; it is either fixed (the second `send` fails as on Rust, held by the test) or `docs/slice.md` states in the channel section that a `send` may succeed until the receiver's end is observed closed, on both targets alike.
- `task gate` green; no row of `docs/quality.md` worse.

## Scope

- `crates/ignite-rt/src` (channels on the LLVM target), `crates/ignite-io/src/wait.rs`, `crates/ignite/tests`. Inferred.
