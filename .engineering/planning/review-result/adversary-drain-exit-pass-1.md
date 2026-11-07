---
format: aep.planning-md/3
id: review-result:adversary-drain-exit-pass-1
kind: review-result
status: active
title: Adversary, drain-exit, pass 1
relations:
- reviews: story:drain-exit
revision: 1
---
unit: story:drain-exit, worktree drain-exit, branch fix/drain-exit, HEAD 4a0a4019 plus uncommitted test files
verdict: CONFIRMED (one red case, also red on base f4232674)
cases: executed 8→10 (`drain::`), red 1
origin: introduced 0 / pre-existing 1 / undecided 0
wrote-outside-worktree: 6 paths under scratch unit-drain-exit/adv1
needs-coordinator: none

## Cases (uncommitted)

- `drain::adversary_process_exit_from_a_task_during_the_drain` (`fixtures/drain_exit_task.ignite`): a spawned task calls `Process.exit(263)` during the drain while `main` idles in a read and a `/bin/sleep 30` child runs; exits 7 within 1 s on both targets, child gone within 2 s. Green.
- `drain::adversary_process_exit_during_the_drain_with_a_stalled_writer` (`fixtures/drain_exit_stall.ignite`): `main` writes `bye\n` to a peer that never reads, a task floods the same stream and blocks in send, `main` calls `Process.exit(3)`. Red on both targets: `still running 5s after `exiting``. Probe: main thread in `futex_do_wait` for the stream's write lock, flood thread in `wait_woken`. Cause: `ignite_io::exit` → `flush()` → `Stream::flush_kept` takes `self.out()` (`crates/ignite-io/src/lib.rs:177-182`, `net.rs:244-250`).

## Suite

`cargo test -p ignite --test main drain::`: with the cases 9 passed, 1 failed; without them 8 passed.

## Findings

- ADV1-1 (pre-existing, warning): `Process.exit` waits forever when another task holds the write lock of a stream the caller has unsent writes on; contradicts "at once" in `docs/slice.md` 16.13 and `ignite_io::exit`'s doc (which exempts stdout for this reason). Reaches a pub/sub server whose broadcast task writes to a subscriber that stopped reading (the pusher). Fix named: `try_lock` or a bounded wait in exit's flush.

## Not broken

Exit-code truncation equal on both targets (263 → 7); exit from a spawned task; `at_exit` hooks kill live children; `ignite_rt_exit` now only called by the generated main; symbol exported; urgent data; the story's tests assert literal values; no std server kind relied on exit waiting.

## Coordinator disposition

ADV1-1 taken into round 2 of this story: the pusher is the program it reaches, and the story's own text promises "at once".

```json
[{"id":"ADV1-1","severity":"warning","title":"Process.exit blocks forever when another task holds the write lock of a stream the caller has unsent writes on","evidence":"drain.rs:537 red on llvm and rust; ignite_io::exit -> flush -> Stream::flush_kept takes out() (ignite-io/src/lib.rs:177, net.rs:244)","test":"crates/ignite/tests/drain.rs::adversary_process_exit_during_the_drain_with_a_stalled_writer + fixtures/drain_exit_stall.ignite","pre_existing":true}]
```
