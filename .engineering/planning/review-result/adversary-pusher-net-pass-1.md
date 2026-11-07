---
format: aep.planning-md/3
id: review-result:adversary-pusher-net-pass-1
kind: review-result
status: active
title: 'Adversary pass 1: pusher-net-builtins'
relations:
- reviews: story:pusher-net-builtins
revision: 1
---
unit: story:pusher-net-builtins, worktree pusher-net at 2d0e7a1 plus my uncommitted test files
verdict: NEEDS-CHANGE
cases: executed 31→49, red 1
origin: introduced 2 / pre-existing 0 / undecided 0
wrote-outside-worktree: 1 (`/home/timo/.cache/scratch/e18a99fc-f99c-4987-93c9-e5de2080b61a/unit-pusher-net/adv1/`, 64K left after `cargo clean`)
needs-coordinator: none

`git diff --stat`: `crates/ignite-io/tests/tcp.rs | 40 +` and one untracked file, `crates/ignite/tests/fixtures/pusher_net/adversary.ignite`. Both are test files; I changed no implementation file.

Lane counts: before = `tcp::` 19, `pusher_net::` 6, `std/net.ignite` 3 per target (my cases left out). After = 21, 6, 6, plus the new fixture's 8 per target.

## Cases (uncommitted)
- `tcp::adv_timed_out_is_the_read_that_ended_last`: **red**, run alone first. Its first run was red for the wrong reason (the test never flushed the write: `left: Eof right: Data([120])`). After adding `server.close()` it fails for the reason claimed: `panicked at crates/ignite-io/tests/tcp.rs:334:5: the read that ended last returned data, yet timed_out is still true`. Red again in the lane run (20 passed, 1 failed, exit 101).
- `tcp::adv_connect_within_waits_its_timeout_and_says_timed_out`: green. It requires the error message `connect 10.255.255.1:80: connection timed out` and a wait of at least 280 ms on a 300 ms timeout.
- `fixtures/pusher_net/adversary.ignite`: 8 cases, all green on Rust and LLVM:
  - an IPv4 peer of a dual-stack `::` listener is written as IPv4
  - `timed_out` is false before any read, and still false after `set_idle_timeout`
  - a partial read after a timeout clears the flag
  - a negative timeout is refused with the contract's message
  - a 100000-day timeout still connects
  - a refused port fails at once
  - `localhost` (two addresses) falls through to the IPv4 one
  - an IPv6 `connect_within` reports the peer without brackets

## Findings
- F1 `crates/ignite-io/src/net.rs:391` (NEEDS-CHANGE, introduced): `read_into` clears `timed_out` before it takes the `reading` lock and never clears it again. Take two tasks reading one stream: reader A times out and sets the flag; reader B, queued behind A, then gets data, but the flag stays `true`. That breaks 26.2: "With two tasks reading one stream, `last` is the read that ended last". What reaches it: two tasks reading clones of one stream, the case 26.2 names. Fix: clear the flag after the lock is taken, or set it to false on `Ok(count)`.
- F2 `crates/ignite/tests/fixtures/pusher_net/connect.ignite:14`, `std/net.ignite` test "connect_within gives up within its timeout" (warning, introduced): both treat any `Err` as success and only check an upper bound on the wait. A `connect_within` that failed at once, or ignored its timeout and used 1 ns, would pass every existing case. The new `adv_connect_within_waits…` case would catch that; I did not run it against a mutated copy. On this host `10.255.255.1` really does time out at about 300 ms (it routes via wg0).
- F3 `crates/ignite-check/src/lib.rs:237` (note, introduced): `peer` and `timed_out` are now reserved names. A record field of fn type called `peer` or `timed_out` on a non-stream record (`hooks.peer(1)`) is now E041 `Hooks has no method peer`; at base `peer` was not in `BUILTIN_METHODS`. The same thing already happens for `local_port` and `urgent_pending`, so the mechanism is not new, but 26 only says the builtin wins on a `TcpStream` receiver. What reaches it: a user program with such a field; none in this repository.

## Not broken
- `peer`/`timed_out` on a `PipeStream` or a `ByteStream`-bounded parameter: a clean E041, no backend crash.
- Effects: `timed_out` without `stream.read` and `connect_within` without `net.connect` both give E016.
- Loader: `peer()` inside a closure, a match arm, an if branch or an interpolation needs no import.
- IPv4-mapped, IPv6, a closed stream's peer, a huge Duration and a negative timeout all behave as specified on both targets.
- The lanes `pusher_net::` (6/6), `std/net.ignite --target=all` (3+3) and `tcp::` apart from F1 are green.

## Coordinator disposition

F1, F2, F3 go to the implementor as round 2 (F3: one sentence in `docs/slice.md` 26). No third adversary pass unless round 2 changes more than `read_into` and the docs.

```json
[
{"id":"F1","severity":"blocker","title":"timed_out stays true after a queued reader's later read returns data","evidence":"crates/ignite-io/src/net.rs:391 clears the flag before the reading lock; tcp.rs:334 panics 'the read that ended last returned data, yet timed_out is still true'","test":"crates/ignite-io/tests/tcp.rs::adv_timed_out_is_the_read_that_ended_last","pre_existing":false},
{"id":"F2","severity":"warning","title":"connect_within tests accept any Err and have no lower bound on the wait","evidence":"crates/ignite/tests/fixtures/pusher_net/connect.ignite:14 and the std/net.ignite inline test match Err(_) only; a mutant ignoring the timeout passes","test":"crates/ignite-io/tests/tcp.rs::adv_connect_within_waits_its_timeout_and_says_timed_out","pre_existing":false},
{"id":"F3","severity":"note","title":"peer and timed_out become reserved: fn-typed record fields of those names are E041","evidence":"crates/ignite-check/src/lib.rs:237; probe hooks.peer(1) gives E041; same mechanism for local_port already","test":"none (scratch probe fnfield.ignite)","pre_existing":false}
]
```
