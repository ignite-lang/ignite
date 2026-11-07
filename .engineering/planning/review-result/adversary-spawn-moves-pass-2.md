---
format: aep.planning-md/3
id: review-result:adversary-spawn-moves-pass-2
kind: review-result
status: active
title: Adversary, spawn-moves-adversary-findings, pass 2
relations:
- reviews: story:spawn-moves-adversary-findings
revision: 1
---
unit: story:spawn-moves-adversary-findings, branch fix/spawn-moves-adversary-findings at 38ec4b51 plus my two uncommitted test files
verdict: CONFIRMED
cases: executed 7→8 (`stream_ownership::`; 7 is the same run with my test left out), red 1 (3 of its 5 inline cases red on LLVM)
origin: introduced 0 / pre-existing 1 / undecided 0
wrote-outside-worktree: 1 directory (`…/unit-spawn-moves/adversary2/`)
needs-coordinator: should `own::keeps` be fixed in this story (`docs/slice.md` now claims "in a later branch" explicitly), or should the claim be narrowed and the case sent to the backlog?

**1. Diff stat**
```
 crates/ignite/tests/stream_ownership.rs | 9 +++++++++
?? crates/ignite/tests/fixtures/streams/nested_spawn_adversary2.ignite
```
Both are test files. No implementation file was touched.

**2. Case added (written before anything ran)**

`stream_ownership::adversary2_later_bindings_of_the_name_agree_on_both_targets` runs `nested_spawn_adversary2.ignite` on both targets and expects 5 passed on each. It is red now.
- **b1, b2, b3**: `spawn fails(server)`, and afterwards the name `server` is bound again by a match arm, a `for` loop or a closure parameter. A failing task must close the end.
- **b4, b5**: the outer `server` is still used after an inner shadowing (`let` in an `if`, or a match arm). The end must stay the body's and stay writable. These two check that the scope-aware `refers` never undercounts and hands over a live binding.

Red output, the case run alone:
```
running 5 tests            (Rust)  ... 5 passed
test tests.streams/b1 a failing task closes a spawned end whose name a later match arm binds ... FAILED (timed out after 20 s)
test tests.streams/b2 a failing task closes a spawned end whose name a later for loop binds ... FAILED (timed out after 20 s)
test tests.streams/b3 a failing task closes a spawned end whose name a later closure parameter binds ... FAILED (timed out after 20 s)
test tests.streams/b4 an end used after an inner shadowing stays the body's and writable ... ok
test tests.streams/b5 an end used after a match arm that binds its name stays the body's and writable ... ok
test result: FAILED. 2 passed; 3 failed; 0 filtered out; finished in 20.00s
test result: FAILED. 0 passed; 1 failed; ... exit 101
```

**3. Suite run** (after the case existed)
- `cargo test -p ignite --test main stream_ownership::`: 7 passed, 1 failed (mine), exit 101. pass-1's cases (`adversary_nested_spawns_close_the_end_on_both_targets`, adv1 and adv3) now pass on both targets, so adv1 and adv3 are fixed.
- `cargo test -p ignite-rt own_`: 2 passed, exit 0.
- `cargo test -p ignite-rt release`: 12 passed, exit 0. This includes `a_release_below_a_mark_leaves_the_mark_valid`.

**4. Finding** (covers 38ec4b51)
- **F1, b1 to b3.**
  - Measured: on LLVM the end stays open, so `rest(client)` hangs; Rust closes it. The cause is in `crates/ignite-llvm/src/own.rs`: `keeps` checks mentions without resolving scopes, so a later binding of the same name makes the outer binding unowned before the new scope-aware `refers` is ever consulted. The places are the closure at :383 (`region::mentions`), the match arms at :461 (the arm's binding is not checked) and the `for` loop at :340 (the loop variable is not checked).
  - Contract drift: `docs/slice.md:6832`, rewritten in this round, now says "a binding that shadows the name, in the same block or in a later branch" holds on both targets.
  - What reaches it: any body that reuses a stream's name later as a match-arm binding, a loop variable or a closure parameter. That is ordinary code, but less common than adv1.
  - Origin: pre-existing. The base compiler (f4261010, built in scratch and then deleted) also times out on b1 to b3 on LLVM.
  - Verdict: CONFIRMED, warning.
  - Fix, named only: make `keeps` skip arm bodies whose pattern binds the name, `for` bodies that bind it and closures that take it as a parameter, the way `refers` already does. The other option is to narrow the doc claim to `let` shadowing.

**5. Attacked and could not break**
- **Owner-list growth.** Handovers happen only at a binding's `let` and never from a loop body. Every block with a registration takes a mark and calls `own_release` on its end, on `break`/`continue` (`release_owned(depth)`), and on `return`/`?` (`release_owned(0)`). So an accept loop's tombstones go with each iteration. The only growth is in recursion, one entry per frame, which is bounded by the stack.
- **Tombstone 0.** `encode` always sets the low word to index+1, so no handle encodes to 0. `release_owner` and `decode` reject 0, so `release_task_handles` and `own_release` skip it.
- **Ordering.** `release` uses `rposition`, and within one task no handle is registered twice, because the task thunk's own registration lives in the child's thread. A spawn that fails to start followed by the handover release still drops exactly two owners.
- **Re-pinned rt test.** It was re-pinned, not relaxed. It now asserts the mark is unchanged (tombstone) and then `mark - 1` after `own_release(mark - 1)`. The old remove behaviour fails both that test and `a_release_below_a_mark_leaves_the_mark_valid`.
- **Scope-aware `refers`.** Using the outer name after an inner `let` shadow (b4) or after a match-arm shadow (b5) correctly blocks the handover on LLVM. `ExprKind` is matched exhaustively, and `pattern_binds` recurses.

**6. Written outside the worktree**
`/home/timo/.cache/scratch/e18a99fc-f99c-4987-93c9-e5de2080b61a/unit-spawn-moves/adversary2/`, which holds:
- `case-alone.log`, `suite.log`, `suite-rt.log`, `suite-rt2.log`
- `base-build.log`, `base-run.log`
- `base-out-llvm/` (7.8 MB)

`base-target/` (697 MB) was created and then deleted.

```findings
- file: crates/ignite-llvm/src/own.rs
  line: 461
  category: contract-drift
  severity: warning
  verdict: CONFIRMED
  origin: pre-existing
  message: a spawned end whose name a later match arm, for loop or closure parameter binds is left unowned by own::keeps (not scope-aware) and stays open on LLVM after the task fails while Rust closes it, against docs/slice.md:6832's "in the same block or in a later branch" claim.
```
