---
format: aep.planning-md/3
id: review-result:adversary-eval-pass-1
kind: review-result
status: active
title: Adversary pass 1, unit eval (wave 11)
relations:
- reviews: story:eval-substrate-model
revision: 1
---
red

Adversary pass 1 on unit eval, `wave/11-eval` at `2e24eccf`, test commit `f4bf8628`.

verdict: red; cases executed 13→14, red 1; origin: introduced 1.

Finding:
1. `crates/ignite-eval/src/eval.rs:1238`, NEEDS-CHANGE, introduced, warning. The model's branch of `builtin_method` evaluates the arguments before the receiver; Rust and LLVM evaluate the receiver first. A test whose receiver and arguments both act on a store or pipe fails in the evaluator while both targets pass it; at base such programs were refused. Red case `eval_model_adversary::adversary_a_model_method_reads_its_receiver_before_its_arguments` (`pick(log, s).put(arg(log), 9)` gives `[2, 1]` against `[1, 2]`).

Not a finding: a panic in a task thread has no `catch_unwind` and would hang the evaluator; no reachable panic found.

Held: 37 probe tests through evaluator, Rust and LLVM agree except the case above; check, plan and explain still give E117 for a property reaching `spawn` and `Channel.bounded`; `ignite plan` output for `terminal.design`, `terminal`, `todo` byte-identical to the goldens; four task-heavy files (80 tests) run 100 to 200 times, four processes at once, one outcome and one fuel count each; 10,000 spawns refused ("a test spawns more than 64 tasks"); wall-clock reads in `crates/ignite-eval` 3 at base and head; `unsafe` 0 and 0; row 1.3 eval builtins 149, patterns 13 at base and head; largest file `eval.rs` 2,732 lines; baseline differs from base only in `eval_cannot_run` (209 removed, 0 added, 39 reasons rewritten); all 209 removed units run (207 pass, 2 fail as Rust does); the head corpus run's refusals are the 153 the baseline lists; top fuel 3,800,043 of 10,000,000.

Not run: mutations of the unit's tests (reasoned by hand); the full differential corpus (the implementor's run was used).

```findings
[
{"file":"crates/ignite-eval/src/eval.rs","line":1238,"category":"contract-drift","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"the model's branch of builtin_method evaluates the arguments before the receiver, Rust and LLVM the receiver first (red case eval_model_adversary::adversary_a_model_method_reads_its_receiver_before_its_arguments: [2, 1] against [1, 2])"}
]
```
