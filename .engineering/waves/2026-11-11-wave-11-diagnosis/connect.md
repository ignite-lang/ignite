# Diagnosis: rows 3.7 and 3.3 terminal/connect-8

Written by the coordinator from the agent's final message (the agent could not write this file).
Evidence in this directory: `out/counts.txt` (every number under its command), `out/check-each.txt`,
`out/test-all7.log`, `fix-1..7-*.patch`, `all-fixes.patch`, `exp/` (copy of HEAD aa1defb2 with the
seven commits on branch `fixes`), scripts `counts.sh`, `runset.sh`, `trace.sh`, `repeat.sh`,
`check-each.sh`, `table.py`, `threads.py`, `sessions.py`, `inherit.rs`.

## Cause (verified by counts)

With 8 clients, head's serving side makes about 5.8 more syscalls per connection than v0.15.0
(14.44 against 8.59 on rust), all in the readiness path. Main is unchanged or cheaper. Per
connection the extras are:

- two failed `recvfrom` (EAGAIN): one before registering, one because `Readiness::wait` returns
  `Woken` right after `EPOLL_CTL_ADD`;
- one `epoll_wait` that returns only EPOLLOUT (0.57 to 0.67);
- one `EPOLL_CTL_DEL` just before `close`;
- a failed `recvfrom` after an eventfd hand-over wake;
- three avoidable `setsockopt`: two on main, plus `SO_RCVTIMEO`.

Inferred, not timed: 8 connections on 4 workers means 62 to 69 % of sessions start on a worker
that already has one. Bursts run serially, so one session's wasted calls are its neighbour's
wait. That fits the host numbers: p50 unchanged, p99 +32 %, +3 us CPU per connection.

Syscalls per connection, base to head (`strace -f -c`, 8 clients x 25): llvm 14.49 to 20.68,
rust 13.62 to 19.43. Head with `IGNITE_IO=threads`: 14.58 (base's pattern). Head with
`IGNITE_WORKERS=8`: 17.93, with 95 % of sessions alone on a worker. Per thread: main 5.25, 5.26,
5.17, 4.78 (base-llvm, head-llvm, base-rust, head-rust); serving threads 9.31, 15.15, 8.59, 14.44.
Three repeats with 8 clients: base-llvm 14.50-14.79, head-llvm 20.49-20.73, base-rust 13.65-13.75,
head-rust 19.19-19.84. Other shapes (same order): 4 clients x 25: 14.52, 17.93, 13.79, 20.06;
50 serial connections: 15.58, 17.58, 14.52, 16.36.

Ruled out: stack allocation (the wave 9 cache works: 1 to 2 mapping calls after the 40th of 200
accepts); cross-thread wakes (0.75-0.93 eventfd writes on head against 0.92-0.94 futex wakes on
base); a lock across accept, work stealing, per-task signal setup, EPOLLONESHOT.

Verified: accepted sockets inherit TCP_NODELAY and SO_OOBINLINE from the listener on kernel
6.18.55, IPv4 and IPv6 (`./inherit`).

## Fixes (crates/ignite-io/src), rust total with 8 clients as each is added

1. `fix-1-listener-options.patch`: `net.rs` `TcpListener::bind_with`, `accept`, `from_socket`: set
   the two options once on the listener. -2 `setsockopt` on main. 17.72.
2. `fix-2-forget-on-drop.patch`: `sched.rs` `Drop for Readiness`, new `release(delete)`;
   `reactor.rs` new `forget`: drop forgets the token, `close` removes the fd from epoll. 16.76.
3. `fix-3-wait-after-register.patch`: `sched.rs` `Readiness::wait` waits after registering
   instead of returning `Woken`. -1 failed `recvfrom`. 15.55.
4. `fix-4-park-after-handover.patch`: `sched.rs` `wait_alone`: a `poll` ended by the eventfd parks
   the task. 14.80.
5. `fix-5-lazy-epollout.patch`: `sched.rs` `Readiness::wait`, new `Descriptor::write_armed`;
   `reactor.rs` new `rearm`, `Interest::with`: no EPOLLOUT until a send waits for room. 14.46.
6. `fix-6-lazy-rcvtimeo.patch`: `net.rs` `read_with`, new `apply_idle`, `store_idle`; `idle.rs`
   `set_idle_timeout`: SO_RCVTIMEO only before a `recv` that blocks. 13.66.
7. `fix-7-first-read-waits.patch`: `net.rs` `send_all`, `read_with`, new `UNREAD` mark: the first
   read of a stream that sent first waits before its `recv`. 12.71.

Totals: 8 clients rust 19.43 -> 12.71 (v0.15.0 13.62), llvm 20.68 -> 13.84 (14.49); 4 clients rust
20.06 -> 13.82 (13.79), llvm 17.93 -> 16.37 (14.52: a hand-over interrupts a lone task's `poll` in
0.72 of connections); serial 50: rust 16.36 -> 13.36 (14.52), llvm 17.58 -> 14.32 (15.58). Three
repeats with 8 clients: rust 12.51-12.66, llvm 13.43-13.77. Each patch applies alone to aa1defb2
and passes `cargo check`; the stack passes the ignite-io tests (112 lib, 192 integration).

Risks: fix 1 depends on the kernel inheriting the options (run `./inherit` on the host); fix 2
relies on stream fds never being dup'd (none in the crate); fix 5 adds one `EPOLL_CTL_MOD` per
stream whose send ever waits for room; fix 7 costs a registration and a wait for a peer that
sends before the greeting. Side observation: `Slot` in `net.rs` declares `listener` before
`readiness`, so a plain drop closes the fd and then calls `EPOLL_CTL_DEL` on a stale fd number;
fix 2 removes that.

Not shown: whether p99 returns to v0.15.0. That needs the dedicated host.
