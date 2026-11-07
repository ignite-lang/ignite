---
format: aep.planning-md/3
id: review-result:adversary-spawn-moves-pass-1
kind: review-result
status: active
title: Adversary, spawn-moves-adversary-findings, pass 1
relations:
- reviews: story:spawn-moves-adversary-findings
revision: 1
---
unit: story:spawn-moves-adversary-findings, branch fix/spawn-moves-adversary-findings at 73d13ab7 plus my uncommitted test files
verdict: NEEDS-CHANGE
cases: executed 6→7 (stream_ownership::), red 1 (2 of its 6 inline cases red on LLVM)
origin: introduced 0 / pre-existing 2 / undecided 0
wrote-outside-worktree: 1 path (a directory tree)
needs-coordinator: two decisions. Is adv3 in scope? The story's Outcome says "a pipe end spawned in a branch … closes … on both targets", but slice.md documents the exception adv3 hits. Is adv1 fixed here or sent to the backlog?

**1. Diff stat** (only test files)
```
 crates/ignite/tests/stream_ownership.rs | 10 ++++++++++
?? crates/ignite/tests/fixtures/streams/nested_spawn_adversary.ignite
```

**2. Cases I added**

The test `stream_ownership::adversary_nested_spawns_close_the_end_on_both_targets` runs `nested_spawn_adversary.ignite` on both targets and expects 6 passed on each. It is red now: Rust passes 6 of 6, LLVM passes 4 of 6. Red output from running this case alone:
```
test tests.streams/adv1 a failing task closes a spawned end whose name only an inner block shadows ... FAILED (timed out after 20 s)
test tests.streams/adv3 a failing task closes a pipe end spawned in a branch that owns a pipe of its own ... FAILED (timed out after 20 s)
test tests.streams/adv4 … match nested in an if ... ok
test tests.streams/adv5 … UFCS receiver spawned inside a branch with named arguments ... ok
test tests.streams/adv6 … else-if chain ... ok
test tests.streams/adv7 named spawn arguments run in source order and arrive in parameter order ... ok
test result: FAILED. 4 passed; 2 failed; 0 filtered out; finished in 20.00s
test result: FAILED. 0 passed; 1 failed; … exit 101
```

- **adv1** spawns `server`, then an `if` branch runs `let (other, server) = Pipe.pair()` and `server.write_all(..)`. LLVM keeps the outer end open, so `rest(client)` hangs. Rust closes it.
- **adv3** is an `if` branch that makes `let (near, far) = Pipe.pair()` and then calls `spawn fails(server)`. LLVM keeps the end open (the documented "block that took a mark" exception) and hangs. Rust closes it.
- **adv4 to adv7** pass on both targets. Against the base compiler (f4261010, built in scratch), adv4 to adv6 are red on LLVM and adv5 and adv7 fail to build on Rust (E0308). So these four cases also catch a revert of the fix.

**3. Suite run**, after the cases existed: `cargo test -p ignite --test main stream_ownership::` gave 6 passed, 1 failed, exit 101. The failure is my case; the 6 that pass existed before it. `cargo test -p ignite-types --test main moves::` gave 14 passed, exit 0.

**4. Findings** (both cover 73d13ab7)
- **F1, adv1.** Measured: `crates/ignite-llvm/src/own.rs:83`. `block_handovers` takes the last statement that mentions the name, and `region::mentions` does not resolve shadowing. The `if` that only touches the inner `server` therefore ends the search and nothing is handed over. Meanwhile `docs/slice.md:6830` claims the handover for "a spawn followed by a binding that shadows the name" on both targets. The two targets disagree.
  - What reaches it: any body that reuses a stream name inside a later branch, which is ordinary code.
  - Origin: pre-existing. The base LLVM build is also red on adv1.
  - Verdict: CONFIRMED, warning.
  - Suggested fix: make the last-mention search scope-aware, so a mention after a rebinding inside a nested block does not count.
- **F2, adv3.** Measured: `crates/ignite-llvm/src/lower.rs:5066`. When the branch took its own owner mark before the spawn, `marks_free` is false and the end is kept. This is documented at slice.md:6834, but it contradicts the story Outcome's "spawned in a branch … closes … on both targets".
  - What reaches it: a branch that opens a pipe or accepts a stream before it spawns, which is a plausible server shape.
  - Origin: pre-existing (base also red).
  - Verdict: NEEDS-CHANGE, warning.
  - Suggested fix: when handing over, leave a tombstone in the runtime's owner list instead of removing the entry, so the marks of nested blocks stay valid and the guard can go.

**5. Attacked and could not break**
- Use after handover or a double close. Every later mention, including shadowed names and closure captures, either blocks the handover or makes the binding unowned.
- A spawn in a loop. It is never searched, so it never hands over.
- The `if` condition or match scrutinee mentioning the end. That blocks the handover.
- `else if` chains, nested `match` inside `if`, and a UFCS receiver with named arguments. All pass.
- Named argument evaluation order (adv7): both targets evaluate in source order and pass in parameter order.
- A plain block statement can't be written in Ignite (`{` parses as a brace literal), so the implementor's "plain nested block" case can't be reached.
- Not run: the 0xDB debug-runtime release check. The handover path doesn't touch arena memory.

Not covered by any case: removing the `marks_free` guard would only make an inner pipe close late, at the outer block's end, and no test I wrote catches that mutant.

**6. Written outside the worktree**
`/home/timo/.cache/scratch/e18a99fc-f99c-4987-93c9-e5de2080b61a/unit-spawn-moves/adversary/`, which holds:
- `base/`: a git archive of f4261010
- `base-target/`: its cargo target
- `base-cargo/`, `cargo/`, `out-llvm/`, `out-rust/`, `base-out-llvm/`, `base-out-rust/`: test build outputs
- `base-probe.ignite`
- `base-build.log`, `case-alone.log`, `suite.log`

`base-target/` is several GB.

```findings
- file: crates/ignite-llvm/src/own.rs
  line: 83
  category: contract-drift
  severity: warning
  verdict: CONFIRMED
  origin: pre-existing
  message: a spawned end whose name a binding in a later branch shadows is not handed over on LLVM (the last-mention search ignores scopes), so it stays open after the task fails while Rust closes it, against slice.md 20.3.1's shadowing claim (adv1).
- file: crates/ignite-llvm/src/lower.rs
  line: 5066
  category: acceptance
  severity: warning
  verdict: NEEDS-CHANGE
  origin: pre-existing
  message: a spawn in a branch that took its own owner mark before the spawn keeps the end on LLVM (marks_free false) while Rust closes it, contradicting the story Outcome for branch spawns (adv3).
```
