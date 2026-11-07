---
format: aep.planning-md/3
id: review-result:adversary-apply-test-deadlines-pass-1
kind: review-result
status: active
title: Adversary, apply-test-deadlines, pass 1
relations:
- reviews: story:apply-test-deadlines
revision: 1
---
unit: story:apply-test-deadlines, branch fix/apply-test-deadlines at 6cbcb66d (base 0ce668d6), working tree with one adversary case added
verdict: NEEDS-CHANGE
cases: executed 4→5, red 1
origin: introduced 0 / pre-existing 2 / undecided 0
wrote-outside-worktree: 7 paths (part 6)
needs-coordinator: yes. The acceptance says deadlines are "named in one place per file". The coordinator must decide whether that covers the files the diff did not touch (finding 2).

**Short answers to your four questions**
1. **Raised deadlines:** none of them weakened an asserted upper bound. `slice18` RESTART (10 s), drain GONE (2 s) and `run.rs` TIMEOUT (15 s) are unchanged. LIVE, SETUP, BUILD, LOADING and the drain, terminal and todo TIMEOUTs are ceilings on waits that return when their event arrives. One wait in the terminal tests is also the only upper bound on a behaviour, and it went from 5 s to 30 s (finding 3, a note). One drain helper cannot fail at all, and before this change it did so after 15 s; now it takes 60 s (finding 1).
2. **Linearity test:** a quadratic regression is still caught (proved by a mutant, below).
3. **Named in one place per file:** only in the files the diff touched (drain, terminal, todo, `slice18` partly). Not in `apply_way_out.rs`, `apply_admission.rs`, `run.rs`, or two waits in `slice18.rs` (finding 2).
4. **Old deadlines left in the story's list:** `apply_way_out.rs:786` (2 s retry on "text file busy") and `run.rs:25` TIMEOUT 15 s are unchanged. For `run.rs`, 15 s is about 4x the slowest alone run (3.79 s), so it meets the 2x rule. Neither is a load failure the story names.

**1. Diff stat**
```
 crates/ignite/tests/drain.rs | 18 ++++++++++++++++++
 1 file changed, 18 insertions(+)
```
It touches only a test file.

**2. Case added**
`crates/ignite/tests/drain.rs`, `drain::assert_gone_fails_while_the_peer_holds_the_connection`. It is marked `#[should_panic]`. It opens a local TCP pair, keeps the peer open and silent, and calls `Session::assert_gone`, which must panic. It is red now. Run alone:
```
test drain::assert_gone_fails_while_the_peer_holds_the_connection has been running for over 60 seconds
test drain::assert_gone_fails_while_the_peer_holds_the_connection - should panic ... FAILED
note: test did not panic as expected at crates/ignite/tests/drain.rs:714:4
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 542 filtered out; finished in 60.25s
```
- **Cause:** `assert_gone` (drain.rs:195-207) treats `Err(_)` as "gone", and a read timeout is an `Err`. It only returns after waiting the full TIMEOUT, so it can never fail.
- **Fix to name (not applied):** return only on `Ok(0)` or `ConnectionReset`, and panic on `WouldBlock` or `TimedOut`, the way `terminal.rs:191 read_until_closed` already does.

**Mutation probe for question 2** (scratch only, no test added)
I copied `moves_scale.ignite` and added a read of `s` after `step(s, i)`, so the move cannot apply and every step copies the state. LLVM, load 6 to 8:
```
5000   finished in 0.03s
20000  finished in 1.42s
50000  finished in 9.24s
200000 test result: FAILED ... finished in 60.00s   (the runner's per-test timeout)
original fixture: 50000 0.00s, 200000 0.01s
```
- With the mutant, the small filter takes 9.24 s, far above the 0.05 s floor. The large one hits the runner's 60 s timeout, so `assert_eq!(status, Some(0))` fails in the warm-up round, before the ratio is ever computed.
- The floor only hides a regression whose 200,000-step time stays under 0.4 s. A real copy per step takes seconds at 50,000 steps. This also measures what the implementor's comment at `slice16.rs:134` had only inferred.

**3. Suite run** (after the case existed)
Command: `cargo test -p ignite --test main -- drain:: slice16::llvm_state_threading_is_linear`, once with `--skip assert_gone_fails` to get the before count.
```
without the case: test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 539 filtered out; finished in 2.98s
with it:          test result: FAILED. 4 passed; 1 failed; 0 ignored; 0 measured; 538 filtered out; finished in 62.61s
```
The script printed `exit 0` both times, but that is grep's exit status because the output was piped. The result lines above are the real outcome. I did not run the whole crate suite; the apply suites take more than 20 minutes.

**4. Findings** (they cover 6cbcb66d)
1. **`crates/ignite/tests/drain.rs:195`, NEEDS-CHANGE, pre-existing.** `assert_gone` cannot fail, as the red case shows. At the base it passed vacuously after 15 s; the diff makes that 60 s.
   - What reaches it: `rollout_drains_and_restarts` at drain.rs:690. That line is the only check that the old session is cut when its descriptor leaks into another process; `assert_no_process` only checks the pid.
2. **`crates/ignite/tests/apply_way_out.rs:786`, NEEDS-CHANGE, pre-existing.** The acceptance item "named in one place per file" is not met in files the diff did not touch:
   - `apply_way_out.rs:786` (2 s) and `:138` (10 s);
   - `apply_admission.rs:298` (10 s);
   - `run.rs:353` (2 s, asserted) and `:478` (5 s read timeout);
   - `slice18.rs:318` (1 s drain of the remaining records after `stop`, which feeds the standings assertion at :486) and `:606` (10 s).

   The 10 s waits are cleanup in `Drop` and fall back to a kill.
3. **`crates/ignite/tests/terminal.rs:1036`, CONFIRMED, pre-existing.** In `a_silent_session_is_closed_after_the_idle_timeout`, only TIMEOUT bounds the idle close from above. That bound went from 5 s to 30 s, about 100x the 300 ms idle timeout. An idle timeout that is 20x too long would have failed before and passes now. There is no lower-than-TIMEOUT assertion of its own. The obvious one would be `waited < SHORT_IDLE_MS + margin`; I did not add it because it is green on today's code.
4. **`crates/ignite/tests/slice16.rs:150`, CONFIRMED, introduced.** With 1 warm-up plus 5 rounds, a quadratic regression slower than the runner's timeout is caught in the warm-up. One that stays under 60 s per run now takes 6 rounds, up to about 6 minutes, instead of 2. The check still holds; only the time to the red result grows.

**5. Attacked and could not break**
- LIVE, BUILD and SETUP are not used as an asserted time bound anywhere. The `elapsed()` uses in `apply_probe_handover.rs`, `rollout_repeat.rs` and `apply_way_out.rs` only go into messages.
- `slice18` RESTART stays at 10 s and is still asserted.
- drain GONE (2 s) is still asserted.
- The drain test's second SIGTERM "at once" is still checked by the `signal() == SIGTERM` assertion, not only by TIMEOUT.
- The todo TIMEOUT and LOADING are pure event waits.
- Row 10 in `docs/slice.md` matches the code.

**6. Paths written outside the worktree**
- /home/timo/.cache/scratch/e18a99fc-f99c-4987-93c9-e5de2080b61a/unit-apply-test-deadlines/adversary/probe/moves_scale.ignite
- /home/timo/.cache/scratch/e18a99fc-f99c-4987-93c9-e5de2080b61a/unit-apply-test-deadlines/adversary/probe/run.sh
- /home/timo/.cache/scratch/e18a99fc-f99c-4987-93c9-e5de2080b61a/unit-apply-test-deadlines/adversary/probe/run.log
- /home/timo/.cache/scratch/e18a99fc-f99c-4987-93c9-e5de2080b61a/unit-apply-test-deadlines/adversary/probe/out/ and .../probe/out-orig/ (the `ignite test` build output)
- /home/timo/.cache/scratch/e18a99fc-f99c-4987-93c9-e5de2080b61a/unit-apply-test-deadlines/adversary/assert_gone.log
- /home/timo/.cache/scratch/e18a99fc-f99c-4987-93c9-e5de2080b61a/unit-apply-test-deadlines/adversary/suite.sh and suite.log

```findings
- file: crates/ignite/tests/drain.rs
  line: 195
  category: mutant
  severity: warning
  verdict: NEEDS-CHANGE
  origin: pre-existing
  message: Session::assert_gone returns on a read timeout, so it passes against a peer that never closes (red case drain::assert_gone_fails_while_the_peer_holds_the_connection), now after 60 s instead of 15 s.
- file: crates/ignite/tests/apply_way_out.rs
  line: 786
  category: acceptance
  severity: warning
  verdict: NEEDS-CHANGE
  origin: pre-existing
  message: deadlines are not named in one place per file in the untouched story files (apply_way_out.rs:786 and :138, apply_admission.rs:298, run.rs:353 and :478, slice18.rs:318 and :606).
- file: crates/ignite/tests/terminal.rs
  line: 1036
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: pre-existing
  message: the idle-close test's only upper bound is TIMEOUT, raised from 5 s to 30 s, so an idle timeout up to about 100x too long now passes.
- file: crates/ignite/tests/slice16.rs
  line: 150
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: six rounds instead of two make a quadratic regression under the runner's 60 s per-test timeout take about three times as long to fail, though the mutant probe shows it is still caught.
```
