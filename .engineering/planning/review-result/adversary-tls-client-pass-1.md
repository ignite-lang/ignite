---
format: aep.planning-md/3
id: review-result:adversary-tls-client-pass-1
kind: review-result
status: active
title: 'Adversary, pass 1: tls-client'
relations:
- reviews: story:tls-client
revision: 1
---
```
unit: story:tls-client, commit 0450ce20 on wave/10-tls, plus my uncommitted test files (worktree w10-tls)
verdict: NEEDS-CHANGE
cases: executed 18→53, red 4
origin: introduced 2 / pre-existing 1 / undecided 0
wrote-outside-worktree: 1 directory (scratch, listed in part 6)
needs-coordinator: decide whether F2 (pre-existing scheduler flush deadlock) is fixed in this wave or sent to the backlog
```

**The main problem:** on the Rust target with the default scheduler (readiness), a TLS session deadlocks when one task writes and the reader task it spawned reads, once the socket buffers fill. The unit names exactly this case as supported. The LLVM target and `IGNITE_IO=threads` pass. Verification, the handshake deadline and truncation all held under every attack.

### 1. `git --no-pager diff --stat`
```
 crates/ignite-io/tests/main.rs | 1 +
 crates/ignite/tests/main.rs    | 1 +
 2 files changed, 2 insertions(+)
?? crates/ignite-io/tests/tls_adversary.rs
?? crates/ignite/tests/tls_adversary.rs
```
Only test paths. No implementation file was touched. Mutants were applied to a copy in scratch.

### 2. Cases added
**`crates/ignite-io/tests/tls_adversary.rs`** (33 tests, 5 of them `child_*` helpers that do nothing in the parent process)
- **Verification, all green:**
  - certificate not yet valid → `certificate refused: not yet valid`
  - self-signed leaf against the system roots, and against another authority → `unknown issuer`
  - chain to an unknown authority → `unknown issuer`
  - IP literal vs a DNS-name certificate, and the reverse → `not valid for …`
  - names refused before any handshake: `""`, `*.x`, `*`, a name with NUL, `/`, a leading space, an inner space, `..`
  - a wildcard certificate covers one label only
  - matching ignores case; a trailing dot does not let another name through
  - authorities holding only whitespace or only a key never fall back to the system roots or to no verification
  - ALPN with no common protocol fails the handshake; a server that offers no ALPN gives `alpn() == ""`
  - a TLS 1.2-only server is verified too
- **Deadline, all green:**
  - a server sending one byte every 50 ms → `handshake timed out after 300 ms`, under 3 s
  - a server that hangs up mid-handshake → `…closed the connection during the handshake`
  - the handshake timeout does not stay on the stream afterwards; an idle timeout set before the handshake is restored
- **ByteStream contract, all green:** a cut without close_notify is an error, on TLS 1.3 and TLS 1.2; a reset is an error; a zero-length write changes nothing; a write after close is refused; 10 MiB in 64 KiB writes beside a reader *thread*.
- **Tasks:**
  - `in_tasks_under_threads`: **green**
  - `in_tasks_under_readiness`: **red**
  - `ten_mebibytes_in_100_kib_writes_in_tasks_under_threads`: **green**
  - `ten_mebibytes_in_100_kib_writes_in_tasks_under_readiness`: **red**
  - `control_ten_mebibytes_over_plain_tcp_in_tasks_under_readiness`: **green**
  - `control_plain_tcp_with_a_short_write_first_in_tasks_under_readiness`: **red**

**`crates/ignite/tests/tls_adversary.rs`**: 8 inline Ignite tests, each run on both targets under both IO modes. `tls_adversary::rust::adversary_cases_run` is **red**; `tls_adversary::llvm::adversary_cases_run` is green.

Red output from the first run of each case alone, before any suite run:
```
tls_adversary::in_tasks_under_readiness   (child_in_tasks)
  left: {"idle": …, "trickle": …, "truncate": …}      <- no "echo" outcome within 20 s
 right: {"echo": "10485760 bytes, in order true, end Ok(())", …}

tls_adversary::child_echo_in_tasks  (TLS, readiness, 10 MiB in 100 KiB writes)
  left: "no outcome within 20s: written 5120000 of 10485760, read 0"
 right: "10485760 bytes, in order true, end Ok(())"

tls_adversary::child_tcp_echo_in_tasks  (plain TCP, a 1-byte write first)
  left: "no outcome within 20s: written 7270400 of 10485760, read 0"
 right: "10485761 bytes, end Ok(())"

tls_adversary::rust::adversary_cases_run
IGNITE_IO=readiness: ExitStatus(unix_wait_status(256))
test main/adversary: a session task and the reader task it spawns carry ten mebibytes ... FAILED (timed out after 60 s)
test result: FAILED. 7 passed; 1 failed; 0 filtered out; finished in 60.01s
```
- **Probe matrix** (readiness): 10 MiB hangs with 1 or 2 workers and with 10 KB or 100 KB writes. 1 MiB passes, because it fits in the loopback socket buffers.
- **Thread stacks under gdb:** the only worker sits in `epoll_wait` with both tasks parked, and the server thread is blocked in `writev`.
- **My own mistake, fixed:** the first run also had `a_chain_to_an_unknown_authority_is_refused` red with `BadSignature`. My two test authorities had the same subject name. With distinct names it is green; it was not a product defect.

### 3. Suite runs (after the cases existed)
```
cargo test -p ignite-io --test main -- tls --skip tls_adversary
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 171 filtered out; finished in 0.20s
cargo test -p ignite-io --test main -- tls
test result: FAILED. 44 passed; 3 failed; 0 ignored; 0 measured; 138 filtered out; finished in 20.06s   exit 101
cargo test -p ignite-rt --test main -- tls          (no adversary file; same both ways)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 27 filtered out; finished in 0.10s
cargo test -p ignite --test main -- tls --skip tls_adversary
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 678 filtered out; finished in 16.04s
cargo test -p ignite --test main -- tls
test result: FAILED. 3 passed; 1 failed; 0 ignored; 0 measured; 676 filtered out; finished in 143.55s   exit 101
```
Executed 18 before (14 + 2 + 2, my files deselected) and 53 after (47 + 2 + 4); 4 red.

**Mutants** (applied to a scratch copy of `crates/ignite-io/src/tls.rs`; "unit suite" is the unit's own `tls::` tests):

| Mutant | Unit suite | Adversary cases |
|---|---|---|
| M1: idle timeout not restored (`:162`) | 14/14 green, **survives** | killed by `the_handshake_timeout_does_not_outlive_the_handshake` and `the_idle_timeout_set_before_the_handshake_is_restored` |
| M2: "not yet valid" reported as "expired" (`:434`) | **survives** | killed by `a_certificate_not_yet_valid_is_refused` |
| M3: a cut without close_notify read as end of stream (`:210`) | **survives** | killed by `a_tcp_close_without_close_notify_…` and `a_tls12_truncation_is_an_error_too` |
| M4: hang-up mid-handshake reported as a timeout (`:388`) | **survives** | killed by `a_server_that_hangs_up_mid_handshake_fails_at_once` |
| M5: each handshake read waits the whole timeout (`:376`) | **survives** | **survives** |
| M6: `verify=false` skips the TLS 1.3 handshake signature (`:591`) | **survives** | **survives** |

### 4. Findings
| # | Severity | file:line | Verdict / origin | What |
|---|---|---|---|---|
| F1 | blocker | `crates/ignite-io/src/tls.rs:369` | NEEDS-CHANGE / introduced | The handshake returns right after `send_pending()` queues the client's last record (Finished). That record stays in the stream's send buffer, and the stream stays on the worker thread's pending-flush list. A reader task on the same worker then flushes that list at its first read and waits for the stream's write lock. The writer holds that lock while parked waiting for socket room. Result: deadlock. |
| F2 | major | `crates/ignite-io/src/net.rs:254` (list), `:425` (`flush_kept`) | CONFIRMED / pre-existing | The root cause, without TLS. The pending-flush list belongs to the worker thread, not the task, so one task's read can block on another task's send. Plain TCP hangs the same way after one short write followed by large writes. |
| F3 | minor | `crates/ignite/tests/tls.rs:101`, `crates/ignite-io/tests/tls.rs:341` | CONFIRMED / introduced | The unit's two "reader and writer share one session" tests cannot fail for this defect. The first writes from the test body, not a task, and 1 MB fits in socket buffers; the second uses OS threads, not tasks. |
| F4 | note | `crates/ignite-io/src/tls.rs:376` | CONFIRMED / introduced | Mutant M5 survives everything. The overshoot is limited to one extra read wait, so at most about twice the timeout. |
| F5 | note | `crates/ignite-io/src/tls.rs:591` | CONFIRMED / introduced | Mutant M6 survives everything: nothing checks that `verify=false` still verifies the handshake signature. Low impact, since that mode trusts any certificate anyway. |

**What reaches F1:** the documented pattern of one task owning the writes and a reader task it spawns (the unit's own docs and commit message, and the NATS/RESP client design), under the default scheduler, once the outgoing data exceeds the socket buffers.
- The Rust target is affected. Its generated `spawn` does not flush (`crates/ignite-rustgen/src/lib.rs:4478`, `:4513`).
- LLVM is not affected in my case: its runtime flushes on spawn (`crates/ignite-rt/src/task.rs:120`). So the same program hangs on one target and passes on the other.
- **Fix to name:** flush at the end of the handshake, before `client` returns.

**What reaches F2:** any task that does a write under 64 KiB, then large writes, while a reader task on the same worker reads, on either target. The origin is pre-existing because every function on that path (`net.rs`, `sched.rs`, `wait.rs`, `task.rs`) is byte-identical at base 61bfcd90 (`git diff --stat 61bfcd90 0450ce20` on them is empty).
- **Fix to name:** `flush_kept` should not wait on a write lock another task holds, or the list should become per-task state.

### 5. Attacked and could not break
- Skipping verification without `verify=false`: name forms, an expired / not-yet-valid / self-signed certificate, an unknown authority, empty authorities, TLS 1.2.
- ALPN: a mismatch fails; no ALPN continues with an empty `alpn()`, which RFC 7301 allows and the docs state.
- Deadline against a silent server, a trickling server and a hang-up, on both targets and both modes.
- Truncation and reset are errors; zero-length write; write after close; drop is not close (same as TCP).
- Feature gate: the check reads the generated code of both crates (program and std), every `TlsStream` comes from a `Tls.client` call written as `::ignite_io::TlsStream::client(`, and Cargo merges features across the build. A program without TLS is covered by the unit's terminal test.
- Using the raw stream after `Tls.client` works on the Rust target (no move error).
- The interpreter (`ignite-eval`) cannot do IO for either stream type, so nothing to compare.

### 6. Written outside the worktree
- `/home/timo/.cache/scratch/2db1a4b7-2d2d-4a62-bab2-d71ac128826e/w10/tls/adv1/` (41 MB): scripts, logs, `mutate.py`, `tls.rs.orig`, and `mut/` (a source copy used for mutants).
- Mutant builds of that copy went into the worktree's `target/`. The end-to-end run wrote `target/tmp/tls_adversary/` and reused the unit's `target/tmp/tls/{rust,llvm}/cargo`.
- Disk now has 41 GB free. The lease is released.

### 7. Findings block
```findings
- file: crates/ignite-io/src/tls.rs
  line: 369
  category: concurrency
  severity: blocker
  verdict: NEEDS-CHANGE
  origin: introduced
  message: the handshake leaves its last record buffered and the stream on the worker thread's flush list, so on the Rust target under readiness a reader task deadlocks against its writer task once socket buffers fill (tls_adversary::rust::adversary_cases_run, tls_adversary::ten_mebibytes_in_100_kib_writes_in_tasks_under_readiness)
- file: crates/ignite-io/src/net.rs
  line: 425
  category: concurrency
  severity: warning
  verdict: CONFIRMED
  origin: pre-existing
  message: the flush list is per worker thread, so a reader task's flush waits on a write lock held by a writer task parked in send; plain TCP deadlocks after one short write (control_plain_tcp_with_a_short_write_first_in_tasks_under_readiness)
- file: crates/ignite/tests/tls.rs
  line: 101
  category: acceptance
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: the shared-session tests (here and crates/ignite-io/tests/tls.rs:341) cannot fail for a reader that never runs, since the writer is not a task or the payload fits in socket buffers
- file: crates/ignite-io/src/tls.rs
  line: 376
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: making each handshake read wait the full timeout instead of the time left survives every suite (overshoot at most about twice the timeout)
- file: crates/ignite-io/src/tls.rs
  line: 591
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: making verify=false skip the TLS 1.3 handshake signature check survives every suite
```
