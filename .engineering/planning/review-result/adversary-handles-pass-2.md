---
format: aep.planning-md/3
id: review-result:adversary-handles-pass-2
kind: review-result
status: active
title: Adversary pass 2, unit handles (wave 11)
relations:
- reviews: story:llvm-release-handle-temporaries
revision: 1
---
red

Adversary pass 2 (last) on unit handles, `wave/11-handles` at `b8aeae5c` (two test commits on top of `0ed34692`; the fix round is `4875cda7`).

verdict: red; cases executed 8→9 (`leak::`), red 1 (`leak::ends_moved_to_a_task_stay_open_for_every_other_holder`: 12 inline cases of `crates/ignite/tests/soak/moves_attack.ignite` per target, Rust 12/12, LLVM 4 passed and 8 failed); origin: introduced 4, pre-existing 4.

LLVM result of each case by compiler (base `f58a7733` / pass 1 `06be2bea` / head `4875cda7`):
- alias made before the call: red / red / red
- record built before the call: red / red / red
- closure that captures the end: red / red / red
- list element read with `xs.at(0)`: red / red / red
- owned end with an alias, then spawned: red / red / red
- arm's own `let a` shadows the block's `a`: red / red / red
- end used after the spawn (kills mutant M3): red / red / green
- match binding of a parameter (kills M5): red / red / green
- field of a parameter (kills M7): red / red / green
- `over_pipe` shape, 100 rounds: green / green / green
- outer block's end: green / green / red (channels and handles 10→100)
- role call's stream: green / green / red (streams and handles 10→50)
Every red use-after-release case fails with `error: 7 is not a channel sender`.

Findings:
1. `crates/ignite-llvm/src/own.rs:204`, CONFIRMED, pre-existing, blocker. `moves_apart` only looks for mentions after the `let u`. An alias, a record or a closure made before it still holds the end, the task takes the only owner, and the holder then uses a released handle (three cases). The rule written in `docs/llvm.md:574` does not hold.
2. `crates/ignite-llvm/src/own.rs:170`, CONFIRMED, pre-existing, blocker. Every builtin result other than `copy()` counts as fresh. `xs.at(i)` returns the list's own element, not a new owner; `docs/llvm.md:572` says it is fresh.
3. `crates/ignite-llvm/src/lower.rs:5737`, CONFIRMED, pre-existing, blocker. An owned binding that is not handed over because it has an alias is still moved to the task without a retain; the block then releases it a second time. `docs/slice.md` 30.3 says the block keeps its owner.
4. `crates/ignite-llvm/src/own.rs:232`, CONFIRMED, pre-existing, blocker. `arm_moves_fresh` resolves a name in an arm's block against the outer block: an arm's own `let a = t` is taken for the block's fresh `a`. The code is new, the base called every `match` fresh, so the outcome was red there too.
5. `crates/ignite-llvm/src/own.rs:207`, NEEDS-CHANGE, introduced, warning. A binding of an outer block is now not fresh, so the task gets a retain, but nothing releases the binding's original owner: one channel and one handle per round. Green at `f58a7733` and `06be2bea`.
6. `crates/ignite-llvm/src/own.rs:179`, NEEDS-CHANGE, introduced, warning. A closure argument is not `plain`, so a role call such as `Pass.serve(listener.accept()?, |x| x)` is never fresh: the task gets a retain and nobody releases the accepted stream's owner. One open stream per round. Green at `f58a7733` and `06be2bea`.
7. `crates/ignite-llvm/src/own.rs:204`, CONFIRMED, introduced, warning. Mutants M3 (drop the later-mention check), M5 (`own.rs:231`, arm binding always fresh) and M7 (`own.rs:202`, non-name argument always fresh) survive the unit's own tests; the new cases `used_after`, `from_option`, `held` kill them. Killed by the unit's tests: M1, M2, M4, M6, M8, M9.
8. `crates/ignite/tests/leak.rs:421`, CONFIRMED, introduced, note. The known-gap test checks that each of the five forms fails by name but not that it fails by growth: a form that turns from a leak into a double release keeps it green.

Held: `over_pipe`: the golden's one added retain is matched on every path; 100 rounds show no growth. Generated IR, pass 1 against head: 211 programs, 1,133 `.ll` files, 0 differ besides the one retain line of `bounds.ignite`. Rows from git objects equal at `f58a7733` and `4875cda7`: 1.3 llvm patterns 46, builtins 13; 7.3 5. Line counts `f58a7733` → `06be2bea` → `4875cda7`: `lower.rs` 6,073 → 6,163 → 6,167; `own.rs` 964 → 1,016 → 1,117; `region.rs` 234 → 234 → 238. Suite: `leak::` 8 passed, 1 failed (the new case); `cargo test -p ignite-llvm` 41 passed, 1 ignored; `task test-ignite` 452 per target; differential slow corpus 890 units, 0 LLVM differences, refusals only on improved lines; clippy and fmt exit 0. Not tested: `copy()` on a `Sender` parameter (E042), named-argument order, recursive callees.

```findings
[
{"file":"crates/ignite-llvm/src/own.rs","line":204,"category":"acceptance","severity":"blocker","verdict":"CONFIRMED","origin":"pre-existing","message":"an alias, record or closure made before `let u = same(a)` still holds the end after the task released its only owner (\"7 is not a channel sender\", 3 cases, red at f58a7733 too)"},
{"file":"crates/ignite-llvm/src/own.rs","line":170,"category":"contract-drift","severity":"blocker","verdict":"CONFIRMED","origin":"pre-existing","message":"`xs.at(0)` counts as a fresh builtin result, so a list element moves to the task and the list's holder uses a released end, against docs/llvm.md:572"},
{"file":"crates/ignite-llvm/src/lower.rs","line":5737,"category":"contract-drift","severity":"blocker","verdict":"CONFIRMED","origin":"pre-existing","message":"an owned binding that is not handed over because it has an alias is still moved without a retain, so block and task both release it, against slice.md 30.3"},
{"file":"crates/ignite-llvm/src/own.rs","line":232,"category":"judgement","severity":"blocker","verdict":"CONFIRMED","origin":"pre-existing","message":"arm_moves_fresh resolves a name bound by an arm's own let against the outer block, so a parameter is moved as if it were the block's fresh end"},
{"file":"crates/ignite-llvm/src/own.rs","line":207,"category":"boundary","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"an outer block's end passed through a call to a task now gets a retain while nothing releases its own owner, leaking one channel and one handle per round (green at f58a7733 and 06be2bea)"},
{"file":"crates/ignite-llvm/src/own.rs","line":179,"category":"boundary","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"a closure argument makes a role call's result not fresh, so `Pass.serve(listener.accept()?, |x| x)` spawned leaks one stream per round (green at f58a7733 and 06be2bea)"},
{"file":"crates/ignite-llvm/src/own.rs","line":204,"category":"mutant","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"mutants M3 (drop the later-mention check), M5 (own.rs:231 arm binding fresh) and M7 (own.rs:202 non-name argument fresh) survive the unit's tests and are killed only by the new moves_attack cases"},
{"file":"crates/ignite/tests/leak.rs","line":421,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the known-gap test checks each form fails by name but not that it fails by growth, so a form that turns from a leak into a double release keeps it green"}
]
```
