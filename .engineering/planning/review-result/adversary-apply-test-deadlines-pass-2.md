---
format: aep.planning-md/3
id: review-result:adversary-apply-test-deadlines-pass-2
kind: review-result
status: active
title: Adversary, apply-test-deadlines, pass 2
relations:
- reviews: story:apply-test-deadlines
revision: 1
---
unit: story:apply-test-deadlines, branch fix/apply-test-deadlines at 3777c1e0 (correction 6cbcb66d..3777c1e0), working tree plus 2 adversary cases
verdict: NEEDS-CHANGE
cases: executed 8→10, red 0
origin: introduced 0 / pre-existing 1 / undecided 0
wrote-outside-worktree: 3 paths (part 6)
needs-coordinator: none

**1. Diff stat**
```
 crates/ignite/tests/drain.rs | 41 +++++++++++++++++++++++++++++++++++++++++
 1 file changed, 41 insertions(+)
```
Only a test file changed.

**Pass 1's findings 1 to 3**
- **Finding 1 (`assert_gone`) is fixed.** It now returns only on `Ok(0)` (end of stream) or `ConnectionReset`, and panics on anything else. The pass-1 case `assert_gone_fails_while_the_peer_holds_the_connection` passes now.
- **Finding 2 (named deadlines) is fixed for the waits I listed.** Every rename keeps its old value: `STOP_ON_DROP` 10 s in three files, `BUSY_RETRY` 2 s, `GONE` 2 s, `ECHO` 5 s. Pass 1 missed one unnamed timing in a story file; see finding A.
- **Finding 3 (idle-close upper bound) is fixed.** The bound is now `IDLE_CLOSE` = 3 s, separate from `TIMEOUT`.

**2. Cases added** (`crates/ignite/tests/drain.rs`; both green, both run alone before the suite)
- **`drain::assert_gone_returns_when_the_peer_closes`:** the peer closes with nothing unread, the server sends FIN, the read sees end of stream. `assert_gone` must return within 5 s.
- **`drain::assert_gone_returns_when_the_peer_resets_after_data`:** the peer has unread client bytes and the client has unread peer bytes when the peer closes, so the kernel sends RST. `assert_gone` must return within 5 s. This is the state a SIGKILL leaves while input is still pending.

Run alone:
```
test drain::assert_gone_returns_when_the_peer_closes ... ok
test drain::assert_gone_returns_when_the_peer_resets_after_data ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 543 filtered out; finished in 0.10s
```
Neither went red: the new `assert_gone` does not panic on a legitimate close.

**3. Suite run** (after the cases existed, load 14.37)
Command: `CARGO_INCREMENTAL=0 cargo test -p ignite --test main -- drain:: silent_session`, exit 0.
```
test drain::assert_gone_returns_when_the_peer_closes ... ok
test drain::assert_gone_returns_when_the_peer_resets_after_data ... ok
test terminal::rust_static::a_silent_session_is_closed_after_the_idle_timeout ... ok
test terminal::rust::a_silent_session_is_closed_after_the_idle_timeout ... ok
test terminal::llvm::a_silent_session_is_closed_after_the_idle_timeout ... ok
test terminal::llvm_static::a_silent_session_is_closed_after_the_idle_timeout ... ok
test drain::an_open_session_survives_sigterm_and_new_connects_are_refused ... ok
test drain::rollout_drains_the_old_process_and_starts_the_new_llvm ... ok
test drain::rollout_drains_the_old_process_and_starts_the_new_rust ... ok
test drain::assert_gone_fails_while_the_peer_holds_the_connection - should panic ... ok
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 535 filtered out; finished in 60.09s
```
The count of 8 before my cases is this same selection without the two `assert_gone_returns_*` cases; I did not run it separately. I did not run the apply suites.

**4. Judgement findings** (they cover 3777c1e0)

**A. `crates/ignite/tests/apply_way_out.rs:991`: NEEDS-CHANGE, pre-existing** (the same line is at base `0ce668d6` line 982).
- **What was measured:** `live_unresolvable` sleeps `Duration::from_secs(2)` ("Its run settles: the failed restart gave `extra` up"). It never checks that the settling happened. It is not a named constant, so the acceptance item "deadlines named in one place per file" is still not met in this file.
- **What reaches it:** `recover_ends_a_live_instance`, the setup of `apply_way_out::llvm_recover_ends_a_live_instance`, one of the tests the story names. Right after the sleep come the assertions `alive(state.server)` and `flow.servers("terminald") == [server]`, then `recover`. Under load the run may not have given `extra` up within 2 s, so those assertions race it. This is inferred; I did not reproduce it, because the test takes about 100 s alone.
- **Fix to name:** wait until the run's journal shows that `extra` was given up, with `LIVE` as the ceiling, or at least name the 2 s at the top of the file.

**B. `crates/ignite/tests/drain.rs:716`: CONFIRMED, introduced (note).**
- **What was measured:** the adopted case is `#[should_panic]` with no `expected`. It waits the full drain `TIMEOUT` before it panics: 60.09 s, the longest test in `drain::`, at load 14.
- **Effect:** every gate now holds one test thread for 60 s, which works against the story's goal. Without `expected`, a panic for another reason (bind, connect) also passes.
- **Fix to name:** set a short read timeout on the stream inside the case instead of `TIMEOUT`, and add `expected = "the session is not gone"`.

**5. Attacked and could not break**
- **`assert_gone` on a legitimate close:** end of stream and RST after data both return (the two cases above). On Linux a read reports no other error kind for a closed peer.
- **Rename values:** all identical to the literals they replace.
- **`REMAINING` 1 s → 30 s:** `ignite run` pipes its children's stdout (`run/process.rs:265`, `run/probe.rs:55`), so its own stdout reaches end of stream when it exits. The drain loop then ends at once, and the 30 s only bounds a reader that lags. The standings assertion now sees all records instead of possibly a cut-off list, which can only make it more accurate.
- **`IDLE_CLOSE` 3 s at load:** passed on all 4 builds at load 14.4. `started` is taken before connect and the prompt read, and the measured close comes after 300.9 to 316.6 ms, so there are about 2.7 s of slack. A flake at load over 15 was not shown.

**6. Paths written outside the worktree**
- /home/timo/.cache/scratch/e18a99fc-f99c-4987-93c9-e5de2080b61a/unit-apply-test-deadlines/adversary2/ (the directory)
- /home/timo/.cache/scratch/e18a99fc-f99c-4987-93c9-e5de2080b61a/unit-apply-test-deadlines/adversary2/cases.log
- /home/timo/.cache/scratch/e18a99fc-f99c-4987-93c9-e5de2080b61a/unit-apply-test-deadlines/adversary2/suite.log

```findings
- file: crates/ignite/tests/apply_way_out.rs
  line: 991
  category: acceptance
  severity: warning
  verdict: NEEDS-CHANGE
  origin: pre-existing
  message: live_unresolvable sleeps an unnamed fixed 2 s for the run to settle before asserting the live server and one terminald, a load race in the setup of llvm_recover_ends_a_live_instance and an unnamed deadline in a story file.
- file: crates/ignite/tests/drain.rs
  line: 716
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: the adopted should_panic case has no expected message and waits the full 60 s drain TIMEOUT before panicking, adding a 60 s test to every gate.
```
