---
format: aep.planning-md/3
id: review-result:adversary-std-nats-client-pass-1
kind: review-result
status: active
title: 'Adversary, pass 1: story:std-nats-client'
relations:
- reviews: story:std-nats-client
revision: 1
---
verdict: findings
unit: story:std-nats-client, worktree w10-nats at 4c076715 plus one untracked test file
cases: executed 40→54 (20→27 per target), red 6 (3 cases × 2 targets)
origin: introduced 3 / pre-existing 0 / undecided 0 (`std/protocol/nats.ignite` does not exist at e6b8e124)
wrote-outside-worktree: 1 directory (part 6)
needs-coordinator: no gate step runs the new fixture (`task test-ignite` only covers std/ and examples/), so moving the 3 red cases inline is for the implementor. Disk was 28 GB after the suite run, below the 30 GB floor, so I stopped building.

1. Diff: `git diff --stat` is empty. The only change is the untracked test file `?? crates/ignite/tests/fixtures/nats_adversary.ignite` (265 lines). No implementation file was touched.

2. Findings. Each case was run alone, executed 1 and filtered out 6, on both targets.
- **F1, blocker, introduced.** `std/protocol/nats.ignite:1083` (`write`: `write_all` with no deadline), together with `keepalive` at :1239 running on the same task.
  - Test: `nats_adversary.ignite:141` "adversary: keepalive loses a TCP connection whose server stopped reading during a publish burst". The client is built as `connect_to` builds it (`dial`, then `start`), with a ping interval of 100 ms and `max_pings_out` 1. It publishes 40 messages of 1 MB to a server that finished the handshake and then stopped reading.
  - Red on rust and llvm: `assert failed at …nats_adversary.ignite:149:3: client.losses.receive_within(3 seconds) == Some(1) … left: None right: Some(1)`.
  - Control at :154 (same deaf server, no publishing) is green: the connection is lost in 0.2 s. So the stuck TCP write is what keeps the keepalive from firing.
  - What reaches it: `connect_to` sets the idle timeout to 0 after the handshake. A partitioned or stalled server with more than about the socket buffers' worth of publishes outstanding is enough. Then no loss is signalled, so there is no reconnect and the outgoing buffer is never used. Once the 1,024-command channel fills, `publish` blocks its caller.
  - This is the root of the deadlock the implementor named. Over `Pipe.pair()` that deadlock cannot occur, because pipe writes never block (a full 16 MiB queue fails the write).
  - Fix: give connection writes a deadline, or move them to a writer task separate from the keepalive.
- **F2, warning, introduced.** `std/protocol/nats.ignite:1110`: `size_of` counts `payload.len()` only.
  - The module's own definition says `payload` is "one message's bytes, its header block included", and slice.md NN.3 says "`pending_bytes` bytes".
  - Test: `:168` "adversary: the outgoing buffer's byte bound counts the header block". Red on both targets: `pending 3, overflow 0: 3 messages of 32 bytes held under a 40-byte bound`.
  - Fix: count `header_block(headers).len()` as well.
- **F3, warning, introduced.** `std/protocol/nats.ignite:855` (also :842): `checked` uses `client.info`, the INFO of the first connection. `attach` (:1288) and `Info(_)` (:1199) never update it.
  - Test: `:188` "adversary: after a reconnect, publishing is checked against the new server's max_payload". Red on both targets: `left: ok right: nats: a message of 9 bytes is larger than the limit of 8`.
  - What reaches it: a reconnect to a cluster node with a smaller `max_payload` or without headers. That server answers with `-ERR` and closes the connection.
  - Fix: keep the current INFO in the session and check the message there.

3. Suite run: `target/debug/ignite test std/protocol/nats.ignite crates/ignite/tests/fixtures/nats_adversary.ignite --std std --target all --out target/ignite-test-adversary --target-dir target/ignite-test/cargo --runtime target/ignite-test/rt/debug/libignite_rt.a` exits 1. Per target: `24 passed; 3 failed; 0 filtered out`, and the three failures are F1, F2 and F3. Each single-case run of a red case exited 1; the 4 probes exited 0 on both targets; `--list` exited 0.

4. Judgement findings (no test):
- J1, note: closing a connection only shuts down its write side (`disconnect` :1270 calls `close`, which is `Shutdown::Write`). After a keepalive loss of a dead peer, the old reader stays blocked in `read` with idle timeout 0 until the kernel's TCP retransmission timeout ends it. That is one task and one socket per such loss.
- J2, note: in `request` (:929), a reply that lands between the end of `receive_within` and `reply.closed()` turns the result into "nats: the client is shut down". It is a race window; I wrote no test because one would be flaky.

5. Attacked and could not break:
- Parser: lowercase operation names, tabs, payloads containing CRLF, zero-length payloads, HMSG size arithmetic, and MSG/HMSG/PING delivered one byte per read (probe :251 green).
- Request/reply: a late reply after the deadline and a reply on a stale sid (probe :232 green).
- `unsubscribe_after` across a reconnect sends `UNSUB 1 2` for the messages left (probe :203 green).
- By reading: the buffer's count bound; a reconnect clears the buffer, so nothing is replayed after a second loss; the 1 s rule uses a strict `>` (:1089), which cannot be tested deterministically at the boundary.
- Subject rules, by reading: wildcards, empty tokens, `>` not last, leading and trailing dots.

6. Written outside the worktree: `/home/timo/.cache/scratch/2db1a4b7-2d2d-4a62-bab2-d71ac128826e/w10/nats/adversary/` (`run-each.sh`, `list.log`, `f1-*.log`, `each-*.log`, `each-exits.log`, `suite.log`). Build output went to `target/ignite-test-adversary` inside the worktree (25 MB).

```findings
[
  {"file": "std/protocol/nats.ignite", "line": 1083, "category": "concurrency", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "the client task blocks in a TCP write_all with no deadline when the server stops reading, so the keepalive on the same task never loses the connection and publish blocks callers once 1,024 commands queue (red: nats_adversary.ignite:141, control :154 green)"},
  {"file": "std/protocol/nats.ignite", "line": 1110, "category": "acceptance", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "the pending_bytes bound counts payload.len() only, though the module defines a message's bytes as including its header block, so header-heavy messages are held past the bound (red: nats_adversary.ignite:168)"},
  {"file": "std/protocol/nats.ignite", "line": 855, "category": "contract-drift", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "publish checks max_payload and headers against the first connection's INFO; after reconnect to a server with a smaller max_payload an oversized message is accepted and sent (red: nats_adversary.ignite:188)"},
  {"file": "std/protocol/nats.ignite", "line": 1270, "category": "judgement", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "disconnect only shuts down the write half, so after a keepalive loss of a dead peer the old reader stays blocked in read with idle timeout 0 until the kernel's TCP timeout"},
  {"file": "std/protocol/nats.ignite", "line": 929, "category": "judgement", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "a reply landing between receive_within's deadline and reply.closed() makes request report 'the client is shut down'; a race window, no deterministic case"}
]
```
