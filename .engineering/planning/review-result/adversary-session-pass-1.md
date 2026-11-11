---
format: aep.planning-md/3
id: review-result:adversary-session-pass-1
kind: review-result
status: active
title: Adversary pass 1, unit session (wave 11)
relations:
- reviews: story:readiness-session-cost
revision: 1
---
red

Adversary pass 1 on unit session (`story:readiness-session-cost`), attacked at `34c4207f`; the pass's case is commit `c6639be7` (`crates/ignite/tests/terminal.rs`). The harness refused the adversary's own findings file; this is its final message, written down by the coordinator.

verdict: red; cases executed 271→272 (ignite-rt 44+29, a main subset 71, `terminal::` 127→128), red 1; origin: introduced 4, pre-existing 3.

Findings:
1. `crates/ignite/tests/terminal.rs:876`, `:883`, CONFIRMED, introduced, major. `first_session_calls` starts `terminald` under `strace -f`; `drop(server)` kills only `strace`, and the traced `terminald --port 0` runs on with parent 1, one per target per run of `cargo test -p ignite --test main`. Red test `terminal::the_count_of_the_first_session_leaves_no_server_running` (`Rust: …/terminald still runs after its session was counted`, same for Llvm; lane 127 passed, 1 failed). Origin: `git log -S strace` on the file gives `34c4207f`.
2. `crates/ignite-io/src/net.rs:666`, CONFIRMED, introduced, major. Mutant survives: without `self.slot.nonblocking.store(true, Ordering::Release);`, `cargo test -p ignite-io` is green, 118 + 245. Reached by a listener accepted from in a task and then off a worker (`crates/ignite/tests/fixtures/drain_bind_kept.ignite`). A killing test (`scheduler::after_a_task_accepted_an_accept_off_a_worker_waits_first`) is in the pass's scratch as `m1h-test.rs.txt`: green at head, red on the mutant with `left: 0 right: 1`; not committed because it is green.
3. `crates/ignite-io/tests/scheduler.rs:859`, `:917`, CONFIRMED, introduced, minor. `a_task_accepts_beside_a_thread_blocked_in_accept` hangs instead of failing: under `set_nonblocking(false)` on a worker the helper panics at `:902` and the test stays in `listener.accept().unwrap()`; ended at 372 s, where older tests go red in about 10 s on the same mutant.
4. `crates/ignite-io/tests/scheduler.rs:848`, CONFIRMED, introduced, minor. `counts.accept_waits, 0` is an absence-only assertion: with the poll restored and `count_accept_wait()` removed the test stays green; the suite goes red only through an incidental `eventfd2` count and the dead-code warning test.
5. `crates/ignite-io/src/drain.rs:97`, `net.rs:159`, CONFIRMED, pre-existing, minor. The `released <address>:<port>` line (`docs/slice.md` 16.13) is lost on SIGTERM to an idle server. Runs without the line, of 100, base against head: Rust readiness 41 and 35, LLVM 8 and 8, Rust threads 27 and 31, LLVM threads 5 and 3.
6. `crates/ignite-rt/src/process.rs:151`, CONFIRMED, pre-existing, minor. An LLVM worker's 64 KiB alternate stack is never unmapped at worker end: 6 workers, 6 mapped, 0 unmapped, at base and head.
7. `crates/ignite/tests/terminal.rs:839`, CONFIRMED, pre-existing, minor. The first session including its worker's start and end, by whole-life `strace -f` difference: v0.15.0 LLVM 28 / Rust 34, base 62 / 65, head 30 / 35. The extra `munmap` at worker end falls after the 300 ms rest, outside the unit's window; its 29 and 26 are right for that window.
8. `crates/ignite-rt/src/process.rs:171`, INFEASIBLE, introduced, note. The restoring `sigaltstack(&before, …)` return is unchecked before `munmap` at `:174`; no LLVM-target thread has a prior alternate stack.

Held: mutants killed (the poll before `accept4`; `prepare()` and each of its three calls; `process.rs` at base; `idle_on_kept` off). SIGTERM (idle, after a session, during a session, twice) and SIGINT on both targets and both IO modes: exit status equal at base and head, lines equal apart from finding 5, descriptors at head at most base's. Overflow at head: in a task after bind reported with SIGABRT on both targets; on main, Rust reports and aborts, LLVM SIGSEGV (not run at base); on a new worker after LINGER, reported. `sigaltstack` per LLVM worker start: base 2, head 1. The unit's count 6 of 6: Rust 29, LLVM 26. No new `unsafe`, no new `#[ignore]`, no test weakened.

Not run: `task gate`, `task quality`, timing, `cargo test -p ignite-io` at unmutated head in the worktree, `test-ignite`, `binary_size`, release rebuilds. Not probed: EMFILE/ECONNABORTED, inherited or double bind, deadlines.

Acceptance bullet 2 ("held by a cell of the path-cost row") cannot be met literally: no such row exists at head; tests hold the counts.

```findings
[
{"file":"crates/ignite/tests/terminal.rs","line":883,"category":"test-leak","severity":"blocker","verdict":"CONFIRMED","origin":"introduced","message":"the helper that counts the first session's system calls starts terminald under strace -f and its drop kills only strace: the traced terminald --port 0 runs on with parent 1, one per target per run of the main test binary; red test terminal::the_count_of_the_first_session_leaves_no_server_running in c6639be7"},
{"file":"crates/ignite-io/src/net.rs","line":666,"category":"missing-test","severity":"blocker","verdict":"CONFIRMED","origin":"introduced","message":"the mutant without self.slot.nonblocking.store(true, Ordering::Release) passes cargo test -p ignite-io (118 + 245); a listener accepted from in a task and then off a worker reaches the line; a killing test exists in the pass's scratch, green at head and red on the mutant with left 0 right 1"},
{"file":"crates/ignite-io/tests/scheduler.rs","line":917,"category":"test-hang","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"a_task_accepts_beside_a_thread_blocked_in_accept stays in listener.accept() with no ceiling under the regression it guards (set_nonblocking(false) on a worker): ended by hand after 372 s where older tests go red in about 10 s"},
{"file":"crates/ignite-io/tests/scheduler.rs","line":848,"category":"weak-assertion","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"counts.accept_waits == 0 asserts an absence only: with the poll restored and the count_accept_wait() call removed the test stays green; the suite goes red only through an incidental eventfd2 count and the dead-code warning test"},
{"file":"crates/ignite-io/src/drain.rs","line":97,"category":"behaviour","severity":"warning","verdict":"CONFIRMED","origin":"pre-existing","message":"the released address:port line of docs/slice.md 16.13 is lost on SIGTERM to an idle server; runs without the line of 100, base against head: Rust readiness 41 and 35, LLVM 8 and 8, Rust threads 27 and 31, LLVM threads 5 and 3"},
{"file":"crates/ignite-rt/src/process.rs","line":151,"category":"leak","severity":"warning","verdict":"CONFIRMED","origin":"pre-existing","message":"an LLVM worker's 64 KiB alternate signal stack is never unmapped when the worker ends: 6 workers, 6 mapped, 0 unmapped, at base and at head"},
{"file":"crates/ignite/tests/terminal.rs","line":839,"category":"measurement","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"the first session with its worker's start and end, by whole-life strace -f difference: v0.15.0 LLVM 28 and Rust 34, base 62 and 65, head 30 and 35; the extra munmap at worker end falls after the 300 ms rest, outside the unit's counting window, whose 26 and 29 are right"},
{"file":"crates/ignite-rt/src/process.rs","line":171,"category":"unchecked-return","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"the return of the restoring sigaltstack is unchecked before the munmap at 174; not reachable, no thread of the LLVM target has a prior alternate stack"}
]
```
