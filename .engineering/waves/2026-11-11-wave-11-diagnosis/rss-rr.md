# Diagnosis: rows 3.3 rr-64 (LLVM) and 3.8 one-connection RSS

Written by the coordinator from the agent's final message (the agent could not write this file).
Patches, scripts and raw outputs are in this directory; `out/` files are named after the command
that made them. Local counts ran at load 45 to 150. LLVM counts use the agent's own
release-runtime links (stripped size within 4.1 KiB of the EC2 record).

## Group B: LLVM rr-64 (row 3.3)

- Shift: llvm/terminal/rr-64 413402 -> 368176 replies/s (-45226); rust 405243 -> 445269 (+40026).
- Cause (verified): a session that has not yet sent anything blocks the worker it is alone on in
  `recv` (`sched::block_alone`, called from `TcpStream::read_with`). Spawns pass blocked workers,
  so the rest of the 64-connection burst lands on the others.
- Placement counts: `probe.py burst`, 64 connections on 4 CPUs, gives connections per worker of
  16/16/16/16, 21/21/21/1, 31/31/1/1 or 61/1/1/1 from round to round, on both targets.
- Per-reply work is equal: syscalls per reply (`strace -f -c`, 1 connection x 500): base-llvm
  3.210, head-llvm 3.290, base-rust 3.250, head-rust 3.336. One `sendto` per reply everywhere, no
  futex per reply, the same runtime entry points per reply in both releases.
- Not noise: head-llvm passes are [666, 871, 861, 991, 892] against base [968-989], and an
  earlier run had the same median (366527).
- Inferred: the EC2 CPU use of 2.92 of 4 CPUs and p50 of 223 us fit two blocked workers. Which
  level each target's median lands on is race timing; nothing counted differs between targets.
- Fix: `B-first-read-does-not-block-its-worker.patch` changes `crates/ignite-io/src/net.rs`
  (`read_with`: only a read whose predecessor emptied the socket may block) and
  `crates/ignite-io/src/sched.rs` (`Readiness::wait` tries `wait_alone` before registering).
- Validation: placement 16/16/16/16 in 6 of 6 rounds on llvm and rust terminal, 4 of 4 on llvm
  todo. The new test is red on HEAD ([1, 1, 1, 13]) and green with the patch; all ignite-io tests
  pass (112 unit, 194 integration).
- Cost of the fix in counts: syscalls per reply, 1 connection 3.290 -> 3.316; 16 connections
  3.484 -> 3.637; syscalls per connect cycle, sequential 15.0 -> 15.5.
- Residual: a connection that made a request and then idles alone still blocks its worker.
- Small rows: llvm/todo/get-1 (-723 replies/s) is noise, the pass ranges overlap. rust/todo/get-1
  (-1927) is real: syscalls per reply are equal, mallocs per reply went 37 -> 53. The new
  `Request.connection` field is the likely source (inference); no patch.

## Group A: peak RSS at one connection (row 3.8)

- Shift: llvm/terminal/rr-1 +212 KiB, llvm/todo/get-1 +252, rust/terminal/rr-1 +100,
  rust/todo/get-1 +240, llvm/todo/patch-16 +60.
- Cause (verified): the executable grew by 164.5 / 184.5 / 94.3 / 129.8 KiB (same row order);
  the scheduler accounts for about 124 KiB of new text.
- Not the cause: anonymous memory is nearly unchanged (terminal +12 to 20 KiB); thread count is
  unchanged on terminal; the 2 MiB task stacks have 20 KiB resident in total.
- Todo only: `keep_holds` adds one live task and worker, +48 to 84 KiB anonymous.
- `A1-llvm-link-as-needed.patch`: `crates/ignite/src/llvm_build.rs` `link()` adds
  `-Wl,--as-needed`, dropping unused libm; returns 330 to 440 KiB locally; LLVM target.
- `A2-http-holds-register-on-demand.patch`: `std/protocol/http.ignite` `serve_listener` starts
  `Holds` only when the application has a `reply`; 28 to 56 KiB, one thread fewer; todo.
- `A3-worker-limit-from-affinity.patch`: `crates/ignite-io/src/sched.rs` `worker_limit` uses
  `sched_getaffinity`; 12 to 20 KiB of file; both targets. It drops cgroup-quota awareness.
- With A1 (local VmHWM): llvm terminal 2788-2796 against base 2964-3084; llvm todo 3092-3116
  against base 3272-3364. EC2 estimate for A1: 190 to 290 KiB (inference).
- Rust rows do not reach v0.15.0 with these: at most about 20 KiB on rust/terminal and 76 KiB on
  rust/todo, against +100 and +240. The rest is the executable's size.
- A1 to A3 are validated by counts only; no test suite was run on them.

All four patches apply to HEAD aa1defb2 together.
