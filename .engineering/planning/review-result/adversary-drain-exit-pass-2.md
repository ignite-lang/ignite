---
format: aep.planning-md/3
id: review-result:adversary-drain-exit-pass-2
kind: review-result
status: active
title: Adversary, drain-exit, pass 2
relations:
- reviews: story:drain-exit
revision: 1
---
unit: story:drain-exit, worktree drain-exit, branch fix/drain-exit, HEAD 7693710 plus uncommitted test files
verdict: NEEDS-CHANGE
cases: executed 10→12 (`drain::`), red 2; ignite-io 83 executed, red 1 (the unit's own test)
origin: introduced 3 / pre-existing 0 / undecided 0
wrote-outside-worktree: scratch unit-drain-exit/adv2 (859 MB)
needs-coordinator: none

## Cases (uncommitted)

Fixtures `flood_exit.ignite`, `flood_return.ignite`: a task writes 32 bytes in a loop to a stream a peer reads at full speed; `main` waits 200 ms, writes `MARK\n`, then ends; 30 runs per target count lost `MARK`.

- `drain::adversary_process_exit_sends_main_s_write_beside_a_busy_writer` (`Process.exit(3)`): llvm lost 2/30, rust 16/30.
- `drain::adversary_main_return_sends_main_s_write_beside_a_busy_writer` (`main` returns `Ok`, no drain): llvm 3/30 and 6/30, rust 0.
- Base `f4232674` (scratch build, 40 runs each): 0/40 on all four binaries.

## Findings

- ADV2-1 (blocker) `crates/ignite-io/src/net.rs:707-718`: `the_exit_flush_skips_a_stream_another_task_is_writing` red 19 of 20 alone: the holder takes the lock before `write_all`, which blocks before `flush_at_exit`. Holds `task gate` red.
- ADV2-2 `net.rs:277-279`: `try_lock` cannot tell a stalled holder from a momentary one; `Process.exit` drops the caller's write beside a busy writer to a reading peer. Fix named: retry until a short deadline (~100 ms).
- ADV2-3 `crates/ignite-rt/src/process.rs` `ignite_rt_exit` and the non-draining `ignite_rt_fail`: LLVM's main return now flushes with `flush_at_exit`, Rust's `ignite_io::run` with the blocking `flush()` (`lib.rs:286`); the targets disagree. Fix named: call `ignite_io::flush()` before `ignite_io::exit` there.

## Not broken

Pass-1 stalled case green; peer not reading; `write_urgent` holds the lock; concurrent close; pipe peer gone; `MSG_DONTWAIT` per call leaves no `O_NONBLOCK`; round-1 write-before-return green; 60,004 bytes to a slow reader arrive; second SIGTERM and concurrent exit (read only).

## Coordinator disposition

All three are fixes inside the story (round 3, implementor), verified by the coordinator; no third adversary pass.

```json
[
{"id":"ADV2-1","severity":"blocker","title":"the unit's own test is red 19 of 20 runs: the holder takes the lock before write_all","evidence":"net.rs:718 'the exit's flush waited'","test":"crates/ignite-io/src/net.rs::tests::the_exit_flush_skips_a_stream_another_task_is_writing","pre_existing":false},
{"id":"ADV2-2","severity":"warning","title":"Process.exit drops the caller's own write when another task holds the lock only for an append","evidence":"HEAD rust 16/30, llvm 2/30; base 0/40; net.rs:277","test":"crates/ignite/tests/drain.rs::adversary_process_exit_sends_main_s_write_beside_a_busy_writer","pre_existing":false},
{"id":"ADV2-3","severity":"warning","title":"main returning Ok without a drain: LLVM drops main's write where Rust sends it","evidence":"HEAD llvm 3/30 and 6/30, rust 0/30; base 0/40; process.rs ignite_rt_exit vs lib.rs:286","test":"crates/ignite/tests/drain.rs::adversary_main_return_sends_main_s_write_beside_a_busy_writer","pre_existing":false}
]
```
