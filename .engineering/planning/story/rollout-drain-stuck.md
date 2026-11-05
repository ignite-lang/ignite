---
format: aep.planning-md/3
id: story:rollout-drain-stuck
kind: story
status: active
title: A third rollout in a row completes after a drain that ended without Released
relations:
- blocks: story:slice-18-records
scope:
- confidence: inferred
  path: std/control.ignite
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-05T16:57:38Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-11-05T16:57:38Z", actor: "human:timo", revision: 4}
---
# Story: rollout-drain-stuck

## Outcome

A rollout whose old server exits on SIGTERM without a `Released` event, or is restarted by the
supervisor while draining, leaves the `Roll` reducer in a state that accepts the next
`Control("rollout")`; a third apply in a row completes.

## Context

- Found by U7 (`story:apply-cycle-timing`, 2026-11-05) on both the slice 18 base c780956661 and
  head 204094d03, terminal design, Rust target: revision 1 and sample 0 apply in 10 to 13 s,
  sample 1 waits 7 to 83 s with `Note("rollout already in progress")`, sample 2 never completes
  (killed at 600 s).
- Suspected path, read from `std/control.ignite`, not traced: `retired()` clears `old_pid` but the
  state stays `Draining`; the supervisor's restart `Started` sets the new server as `old_pid`; the
  120 s timer SIGKILLs it and clears `old_pid`, the state stays `Draining` with no timer, and every
  later `Control("rollout")` is refused.
- Evidence: U7's host logs (devtools scratch `u7/perf2/evidence/`).

## Acceptance

- An inline test in `std/control.ignite` steps the event sequence above and shows the reducer
  back in a state that accepts `Control("rollout")`; red before the fix, green after, both targets.
- An end-to-end test applies three revisions in a row to one host on the terminal design and each
  apply exits 0.
- `task gate` green.

## Out of Scope

- The 22.15 timing method; it stays as written.
