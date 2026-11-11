---
format: aep.planning-md/3
id: story:readiness-session-cost
kind: story
status: active
title: A connection and a session in readiness mode cost no more than a thread did
relations:
- informed_by: story:binary-size-back
- informed_by: story:path-cost-counted-at-every-merge
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-11T18:03:55Z", actor: "human:timo", revision: 2, executor: "agent:claude-coordinator", correlation: "wave-11"}
- {from: "proposed", to: "active", at: "2026-11-11T18:03:55Z", actor: "human:timo", revision: 3, executor: "agent:claude-coordinator", correlation: "wave-11"}
---
# Story: a connection and a session in readiness mode cost no more than a thread did

## Outcome

The user-space cost of one connection and of one `terminald` session in readiness mode is at or below what v0.15.0 spent with a thread per connection, or the difference is shown to be paid back in time on the dedicated host.

## Why

Adversary pass 1 on unit size of wave 11 (`review-result:adversary-size-pass-1`, finding 9), pre-existing since the readiness scheduler of waves 9 and 10:

- LLVM instructions per connection: 22,022 at v0.15.0, 36,055 at the unit's base, 35,506 at its head.
- LLVM `terminald` session: 40,240 instructions and 25 system calls at v0.15.0; 73,403 and 63 at the unit's head.

These are user-space counts. v0.15.0 ran a thread per connection, whose cost in the kernel they do not show; rows 3.3, 3.7 and 3.8 on the dedicated host judge the time.

Candidates measured in wave 11 and not taken, to be judged with the per-merge path-cost row of `story:path-cost-counted-at-every-merge`:

- An epoll slab by token with its free list in the slots: LLVM start about 1,400 instructions lower and 43 lower per connection, with a layout change in `ignite_io::text::find` that moved the request count by about 150 either way.
- Rust requests in threads mode are 226 instructions above the head before fix round 2 of unit size (131,094 against 130,868); cause not found.

## Acceptance

- The session's 63 system calls are itemized by call and by the code that makes them, against the 25 of v0.15.0.
- Each count that is brought down is held by a cell of the path-cost row.
- Rows 3.3, 3.7 and 3.8 of the next dedicated-host run are not worse.
- `task gate` green; no row of `docs/quality.md` worse.

## Scope

- `crates/ignite-io/src/sched.rs`, `crates/ignite-io/src/reactor.rs`, `crates/ignite-io/src/run.rs`. Inferred.
