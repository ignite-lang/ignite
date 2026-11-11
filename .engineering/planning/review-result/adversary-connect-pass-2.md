---
format: aep.planning-md/3
id: review-result:adversary-connect-pass-2
kind: review-result
status: active
title: Adversary pass 2, unit connect (wave 11)
relations:
- reviews: story:readiness-syscalls-per-connection
revision: 1
---
red

Adversary pass 2 (the last) on unit connect (`story:readiness-syscalls-per-connection`), branch
`wave/11-connect` at `623035a4`, fix round `34cdecc6..623035a4`. Source: the adversary agent's
report to the coordinator, 2026-11-11; the adversary committed nothing.

Three findings. The coordinator's reading of the third, which the adversary left undecided: it
is the story's own acceptance (a connection costs no more system calls than at v0.15.0), the
dedicated host sets no worker count, and the reactor made by every started worker is
introduced by the unit; it went back to the implementor with the other two.

What held:

- The count test, 25 of 25 rounds green under load: 8 clients x 25 ran 11.01 to 11.77, 50 in
  sequence 11.82 to 11.94; `accept4` 1.00 and `recvfrom` 1.00 per connection, no `setsockopt`.
- The two pass-1 mutations the fix round was to catch (`EPOLL_CTL_DEL` on drop, eager
  `EPOLLOUT`) go red 2 of 2.
- No fixed port. The warnings test uses its own target directory and passes without a network.
- The source diff of the fix round is the `cfg` on `idle_ms` and its doc comment.
- With four workers, every call counted: head 12.07 and 12.29 against v0.15.0's 13.61 and
  13.68 (Rust target, 8 clients x 25).

Counting every call at the default worker count (20 CPUs), release `terminald`, Rust target,
8 clients x 25, interleaved: v0.15.0 13.66 to 14.16 (median 13.76, 8 to 9 workers started per
200 connections); the unit's base 14.76 to 15.06 (16 to 17); head 13.33 to 15.09 (median 14.24,
9 to 17).

```findings
[
{"file":"crates/ignite-io/tests/scheduler.rs","line":1055,"category":"mutant","severity":"blocker","verdict":"NEEDS-CHANGE","origin":"introduced","message":"the reworked count test stays green when eager SO_RCVTIMEO, the failed first recv or the missing poll before accept4 comes back, and only sometimes catches the missing read-back of stale wakes; the guard test adv2/guard-test.rs catches all four"},
{"file":"crates/ignite-io/tests/scheduler.rs","line":1313,"category":"judgement","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"when a client panics, strace and the counted server keep running, adopted by init and blocked in accept, and the trace file is never removed"},
{"file":"crates/ignite-io/tests/scheduler.rs","line":1127,"category":"acceptance","severity":"blocker","verdict":"CONFIRMED","origin":"undecided","message":"counting every call at the default worker count, head's terminald makes more calls per connection than v0.15.0 (Rust median 14.24 against 13.76, LLVM 14.93-16.16 against 14.49) because of extra worker starts and a reactor per worker, which the test's call set and its fixed 4 workers leave out"}
]
```
