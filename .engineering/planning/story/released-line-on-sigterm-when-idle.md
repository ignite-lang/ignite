---
format: aep.planning-md/3
id: story:released-line-on-sigterm-when-idle
kind: story
status: draft
title: An idle server that gets SIGTERM always prints its released line
relations:
- informed_by: story:readiness-session-cost
revision: 1
---
# Story: an idle server that gets SIGTERM always prints its released line

## Outcome

A server that is ended by SIGTERM while no session runs prints the `released <address>:<port>` line of `docs/slice.md` 16.13 in every run, on both targets and in both IO modes.

## Why

Found by the adversary pass on unit session (wave 11, `review-result:adversary-session-pass-1`, finding 5, pre-existing): at `crates/ignite-io/src/drain.rs:97` and `net.rs:159` the line is lost in part of the runs. Runs without the line, of 100, at the unit's base and head: Rust readiness 41 and 35, LLVM readiness 8 and 8, Rust threads 27 and 31, LLVM threads 5 and 3. The unit did not change the rate.

## Acceptance

- 200 runs per target and IO mode of "start, wait for the listening line, SIGTERM" print the line 200 times. Held by a test that fails at today's head.
- The exit status and the other lines of a drain are those of today (the adversary's table: SIGTERM idle, after a session, during a session, twice; SIGINT).
- `task gate` green; no row of `docs/quality.md` worse.

## Scope

- `crates/ignite-io/src/drain.rs`, `crates/ignite-io/src/net.rs`, `crates/ignite/tests/terminal.rs`. Inferred.
