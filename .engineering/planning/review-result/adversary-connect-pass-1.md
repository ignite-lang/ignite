---
format: aep.planning-md/3
id: review-result:adversary-connect-pass-1
kind: review-result
status: active
title: Adversary pass 1, unit connect (wave 11)
relations:
- reviews: story:readiness-syscalls-per-connection
revision: 1
---
red

Adversary pass 1 on unit connect (`story:readiness-syscalls-per-connection`), branch
`wave/11-connect` at `34cdecc6`, base `f58a7733`. Source: the adversary agent's report to the
coordinator, 2026-11-11; the adversary committed nothing.

Executed 308 tests on the head, all passing. Four findings, all introduced by the unit; none
pre-existing.

What held, each checked by a mutation in a copy:

- The burst test goes red with the first-read change undone (`[1, 1, 1, 13]`).
- The count test goes red when eager `SO_RCVTIMEO`, the failed first `recv`, the missing poll
  before `accept4` or the missing read-back of stale worker wakes comes back.
- Threads mode: 112 + 196 passed with `IGNITE_IO=threads`.
- No `unsafe` site, dependency, `quality/` change or ignored test added.
- The count claim by a second method (release `terminald`, Rust target, `strace -f -c`,
  interleaved with the v0.15.0 binary, 3 rounds): 8 clients x 25, head 12.67 / 12.78 / 12.91
  against 13.62 / 13.68 / 13.70.

Not established: the LLVM target's count; reuse of a descriptor number after a drop without
`EPOLL_CTL_DEL` rests on reading the code, no test.

The findings went back to the implementor, who answered them in `623035a4`.

```findings
[
{"file":"crates/ignite-io/tests/scheduler.rs","line":1,"category":"test","severity":"blocker","verdict":"needs-revision","origin":"introduced","message":"a_connection_costs_no_more_system_calls_than_at_v0_15_0 fails on the unchanged head in 2 of 20 rounds (15.14 and 14.64 per connection against the bound 14.52): one-time worker-thread setup is inside the count and varies with machine load; it also fails where strace is absent or ptrace is blocked"},
{"file":"crates/ignite-io/src/net.rs","line":949,"category":"correctness","severity":"warning","verdict":"needs-revision","origin":"introduced","message":"method idle_ms is never used: every Rust-target build of a program prints the warning, because the only non-test caller left with the change"},
{"file":"crates/ignite-io/tests/scheduler.rs","line":1,"category":"test","severity":"warning","verdict":"needs-revision","origin":"introduced","message":"no test fails when EPOLL_CTL_DEL on drop or eager EPOLLOUT is put back: all 308 tests pass and the count stays under its bound (13.07 and 12.68)"},
{"file":"crates/ignite-io/src/sched.rs","line":1377,"category":"test","severity":"warning","verdict":"needs-revision","origin":"introduced","message":"the path that adds room-to-write to an existing registration has no test: with it removed all 308 tests pass and a writer on a socket already registered for reading stalls after 2625024 bytes"}
]
```
