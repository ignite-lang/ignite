---
format: aep.planning-md/3
id: review-result:adversary-eval-pass-2
kind: review-result
status: active
title: Adversary pass 2, unit eval (wave 11)
relations:
- reviews: story:eval-substrate-model
revision: 1
---
red

Adversary pass 2 (the last) on unit eval, `wave/11-eval`, fix round `f4bf8628...3879d1aa`, test commit `82e2c9af`.

verdict: red; cases executed 48→49, red 1; origin: introduced 3.

Findings:
1. `crates/ignite-eval/src/eval.rs:1273`, NEEDS-CHANGE, introduced, warning. Under the model a plain builtin still evaluates its arguments before its receiver (`text_of(log).starts_with(prefix_of(log))` logs `[2, 1]` in the evaluator, `[1, 2]` on Rust and LLVM); refused at base. Red case `eval_model_adversary::adversary_a_plain_builtin_reads_its_receiver_before_its_arguments_under_the_model`.
2. `crates/ignite-eval/src/eval/world.rs:63`, INFEASIBLE, introduced, note. `resume_unwind` runs before the model is cleared: after a caught body panic a later compile-time call sees "the test is over". No reachable panic and no catching caller found; no test holds the body-panic `catch_unwind`.
3. `world.rs:33`, INFEASIBLE, introduced, note. A panic in the supervisor thread hangs the test (exit 124 under an injected panic); no reachable panic there found.

Held: 11 of 12 order probes agree across evaluator, Rust and LLVM (arguments that sleep, spawn, send or close; a receiver failing by `?`, an index out of range, overflow in a task; a receiver through a field; a chained call); the branch is chosen by the static builtin, so no receiver is evaluated twice. A task panic while the body or other tasks wait on a channel, a pipe, a full send or a sleep ends with `evaluation failed: a task of the model panicked: …` and one fuel count, 3 runs of 3; the next test in the same evaluator passes; thread count 2 before and after 210 tests. `cargo test --workspace` gains one test from the removed `test = false`; rows 7.2 = 27 and 7.3 = 5 do not move. Four files (80 tests) × 100 runs, four processes at once: one outcome and one fuel count each. Row 1.3 eval builtins 149, patterns 13; `unsafe` 0; wall-clock reads 2 at base and head; largest file `eval.rs` 2,733 lines. `cargo test -p ignite-eval` exit 0; clippy and fmt exit 0.

Not probed: a role receiver.

```findings
[
{"file":"crates/ignite-eval/src/eval.rs","line":1273,"category":"contract-drift","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"under the model a plain builtin still evaluates its arguments before its receiver (red case eval_model_adversary::adversary_a_plain_builtin_reads_its_receiver_before_its_arguments_under_the_model: [2, 1] in the evaluator, [1, 2] on Rust and LLVM)"},
{"file":"crates/ignite-eval/src/eval/world.rs","line":63,"category":"judgement","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"resume_unwind runs before the model is cleared, so after a caught body panic a later compile-time call sees the ended test; no reachable panic found and no test holds the body-panic catch_unwind"},
{"file":"crates/ignite-eval/src/eval/world.rs","line":33,"category":"judgement","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"a panic in the supervisor thread hangs the test (exit 124 under an injected panic); no reachable panic there found"}
]
```
