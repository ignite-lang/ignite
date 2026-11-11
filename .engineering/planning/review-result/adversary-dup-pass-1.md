---
format: aep.planning-md/3
id: review-result:adversary-dup-pass-1
kind: review-result
status: active
title: Adversary pass 1, unit dup (wave 11)
relations:
- reviews: story:duplicated-rules-back
revision: 1
---
red

Adversary pass 1 on unit dup, `wave/11-dup` at `783db0e3` (the unit's commits `572e7ae8`, `55e8743c`, `05d90203` merged with the integration branch at `2e57c8c8`), test commits `5abaa86e`, `ed10429f`.

verdict: green (one red case, pre-existing); cases executed 593→601, red 1; origin: introduced 1, pre-existing 3, undecided 5.

Findings:
1. `crates/ignite-check/src/lib.rs:4069`, CONFIRMED, pre-existing, warning. A lowercase constant in an or-pattern (`limit | 0`) is refused with E049 against `docs/slice.md` row 5 ("whatever its case"), because the binding walk lists constant names; the old walks did the same and the binary built from head minus the unit gives the same E049. Red case `or_patterns::a_lowercase_constant_in_an_or_pattern_is_no_binding`.
2. `crates/ignite-io/src/net.rs:359`, CONFIRMED, pre-existing, warning. `layering::compiler_crates_name_no_application_protocol` (`cargo test -p ignite-check`) is red on the merged head because of comments from integration commits `fb731c8e` and `ee9aaf87` (unit reply; in its fix round).
3. `crates/ignite-types/Cargo.toml:11`, CONFIRMED, pre-existing, note. `[lib] test = false` since `7c6da61d` means the 3 tests of `builtins.rs` added by `39009e0f` never run; they pass with `--lib`.
4. `crates/ignite-eval/src/eval.rs:1536`, INFEASIBLE, introduced, note. The last arm now reports a receiver-value mismatch of a handled builtin as "has no value at compile time" instead of naming the receiver; every one of the 149 builtins is a static, a direct method, an arm, or one of the 55 deleted names, so no typed program reaches it.
5. to 9. Mutants that survived the suite, origin undecided (not run against the old walks), each now killed by an adversary case: `crates/ignite-check/src/lib.rs:4069` `.first()` to `.last()` (`or_patterns::e049_names_the_first_binding_of_each_alternative`); `crates/ignite-types/src/moves.rs:1328` unbind only the first name (`moves::every_name_a_loop_pattern_binds_moves`); `crates/ignite-llvm/src/own.rs:349` `pattern_names` reads one name (`owned_patterns::a_match_whose_second_binding_escapes_owns_no_handle`); `crates/ignite-llvm/src/region.rs:156` `pattern_binds` always false (`owned_patterns::a_match_arm_that_binds_a_name_again_hides_the_outer_binding`); `crates/ignite-rustgen/src/lib.rs:3432` `lent_let` ignores `let` patterns (`lent_rebinding::a_let_pattern_that_binds_the_bytes_again_keeps_them_owned`).

Held: the counter is not gamed (no glob import, alias, macro or re-export added; bare-word builtin names per crate at base → head: llvm 149 → 149, rustgen 58 → 54, eval 149 → 124). Row 1.3 base → head: rust/patterns 83 → 65, rust/builtins 57 → 53, llvm/patterns 46 → 23, llvm/builtins 13 → 13, eval/patterns 13 → 13, eval/builtins 149 → 124. Generated code and diagnostics identical between head and head minus the unit across all 643 tracked `.ignite` files: `check` output and exit codes, `emit` Rust of 329 programs, 767 build `.ll` files, 993 test-build `.ll` files, 993 test-build `.rs` files, `plan` output. `Pattern::bindings` equals each old walk for every `PatternKind`, order and duplicates included; the one difference (rustgen descending into range bounds) is unreachable (`parser.rs:2939`, E045). `is_scalar_to_text` sets identical at all three rustgen sites. The 55 deleted names are still refused with the old text when no model runs; the model handles 30 of them only inside a test. 7.2 31 → 31 `#[ignore`, `unsafe` 263 → 263, 7.3 the same 5 files, `ast.rs` 1,081 → 1,120 lines. Slow corpus refused only on improvements: 1.1 eval 4 → 2, 1.2 eval 494 → 708, 1.2 llvm and rust 856 → 861, 1.6 eval 362 → 153, 4.3 eval 234 → 365; `eval_cannot_run` 153 entries equal to the baseline's. Suite: 508 passed over five crates; `cargo test -p ignite-check` 33 passed, 2 failed (finding 1's case and finding 2); `eval:: eval_model leak:: plan:: typed_json::` 58 passed; clippy and fmt exit 0.

```findings
[
{"file":"crates/ignite-check/src/lib.rs","line":4069,"category":"acceptance","severity":"warning","verdict":"CONFIRMED","origin":"pre-existing","message":"a lowercase constant in an or-pattern (limit | 0) is refused with E049 against docs/slice.md row 5, because the binding walk lists constant names; red case or_patterns::a_lowercase_constant_in_an_or_pattern_is_no_binding"},
{"file":"crates/ignite-io/src/net.rs","line":359,"category":"contract-drift","severity":"warning","verdict":"CONFIRMED","origin":"pre-existing","message":"ignite-check layering::compiler_crates_name_no_application_protocol is red on the merged head because of comments from integration commits fb731c8e and ee9aaf87"},
{"file":"crates/ignite-types/Cargo.toml","line":11,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"[lib] test = false since 7c6da61d means the 3 builtins.rs tests added by 39009e0f never run; they pass with --lib"},
{"file":"crates/ignite-eval/src/eval.rs","line":1536,"category":"judgement","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"the last arm now reports a receiver-value mismatch of a handled builtin as having no value at compile time instead of naming the receiver; no typed program reaches it"},
{"file":"crates/ignite-check/src/lib.rs","line":4069,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"undecided","message":"the mutant .first() to .last() survived the suite; killed by or_patterns::e049_names_the_first_binding_of_each_alternative"},
{"file":"crates/ignite-types/src/moves.rs","line":1328,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"undecided","message":"the mutant that unbinds only the first name survived; killed by moves::every_name_a_loop_pattern_binds_moves"},
{"file":"crates/ignite-llvm/src/own.rs","line":349,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"undecided","message":"the mutant where pattern_names reads one name survived; killed by owned_patterns::a_match_whose_second_binding_escapes_owns_no_handle"},
{"file":"crates/ignite-llvm/src/region.rs","line":156,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"undecided","message":"the mutant where pattern_binds is always false survived; killed by owned_patterns::a_match_arm_that_binds_a_name_again_hides_the_outer_binding"},
{"file":"crates/ignite-rustgen/src/lib.rs","line":3432,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"undecided","message":"the mutant where lent_let ignores let patterns survived; killed by lent_rebinding::a_let_pattern_that_binds_the_bytes_again_keeps_them_owned"}
]
```
