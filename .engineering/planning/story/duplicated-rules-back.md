---
format: aep.planning-md/3
id: story:duplicated-rules-back
kind: story
status: implemented
title: The backends name no more patterns and builtins than at v0.15.0
relations:
- depends_on: story:eval-substrate-model
- depends_on: story:llvm-release-handle-temporaries
- informed_by: story:release-refuses-every-worse-row
scope:
- confidence: inferred
  path: crates/ignite-check/src/lib.rs
- confidence: cited
  path: crates/ignite-eval/src/eval.rs
- confidence: cited
  path: crates/ignite-llvm/src/own.rs
- confidence: cited
  path: crates/ignite-llvm/src/region.rs
- confidence: cited
  path: crates/ignite-rustgen/src/lib.rs
- confidence: cited
  path: crates/ignite-syntax/src/ast.rs
- confidence: cited
  path: crates/ignite-types/src/builtins.rs
- confidence: cited
  path: crates/ignite-types/src/lib.rs
- confidence: inferred
  path: crates/ignite-types/src/moves.rs
- confidence: inferred
  path: crates/ignite-types/src/typer.rs
- confidence: inferred
  path: docs/quality.md
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-11T05:01:10Z", actor: "human:timo", revision: 4, executor: "agent:claude-coordinator", correlation: "wave-11"}
- {from: "proposed", to: "active", at: "2026-11-11T05:01:10Z", actor: "human:timo", revision: 5, executor: "agent:claude-coordinator", correlation: "wave-11"}
- {from: "active", to: "implemented", at: "2026-11-11T23:22:42Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}}
---
# Story: the backends name no more patterns and builtins than at v0.15.0

## Outcome

Row 1.3 is at or below its v0.15.0 value in every cell: eval/builtins 133 (now 149),
llvm/patterns 30 (now 46), rust/builtins 54 (now 57); the three cells that are not worse
(eval/patterns 13, llvm/builtins 13, rust/patterns 83) do not rise.

## Why

Timo, 2026-11-10: "every release should be an improvement in quality, not even allowed to
accept 10% degregation". Row 1.3 is a hill-climb row, which the release check skipped; the
adversary's pass on `story:release-refuses-every-worse-row` (2026-11-11) showed it worse than
v0.15.0 in three cells. The release check now judges it.

Diagnosis (`.engineering/waves/2026-11-11-wave-11-diagnosis/dup.md`): the sixteen new
`PatternKind::` mentions of the llvm crate are a third and fourth copy of "the names a pattern
binds"; the rust cell grew by three names in lists that repeat a rule the typer owns; the eval
cell grew by ten conversions it evaluates and six names it lists only to refuse them.

## Acceptance

- `quality::static_metrics_hold_the_bar` measures eval/builtins <= 133, llvm/patterns <= 30,
  rust/builtins <= 54, and no other cell of 1.3 above its value at the story's base.
- The counter is unchanged. No name leaves by `use Builtin::*` or `use PatternKind::*`, an
  alias, a macro that spells the names, or a move into a file or crate the counter does not
  read. The llvm crate names no more builtin variants as bare words than at the story's base.
- Each removal moves a rule to one place: a method on the type in the crate that defines it
  (`Pattern` in `ignite-syntax`, `Builtin` in `ignite-types`), used by every consumer that
  repeated it, or the deletion of a list that a fallback already covers.
- Generated code of both targets is byte-identical for the golden tests and the examples, or
  the report names each difference and why it is right.
- The evaluator refuses what it refused: the entries of `eval_cannot_run` and their reasons in
  `quality/baseline.json` are unchanged, a builtin without a value at compile time is still a
  refusal of the same kind (not a different error class), and `check`, `plan` and `explain`
  print the same diagnostics for the cases that pin them.
- `cargo test -p ignite-syntax -p ignite-types -p ignite-eval -p ignite-llvm -p ignite-rustgen`,
  the `leak::` tests and `differential::` tests of `ignite` pass.

## Scope

- `crates/ignite-syntax/src/ast.rs` (`impl Pattern`). Cited.
- `crates/ignite-llvm/src/own.rs`, `region.rs`. Cited.
- `crates/ignite-types/src/lib.rs` or `builtins.rs` (`impl Builtin`). Cited.
- `crates/ignite-rustgen/src/lib.rs` (`fresh_text`, `format_part`, `method`, `pattern_binds`,
  `pattern_bindings`). Cited.
- `crates/ignite-eval/src/eval.rs` (`builtin_value`, `show`). Cited.
- `crates/ignite-types/src/typer.rs`, `moves.rs`, `crates/ignite-check/src/lib.rs`,
  `docs/quality.md`. Inferred.
- Shares `crates/ignite-eval/src/eval.rs` with `story:eval-substrate-model` and
  `crates/ignite-llvm/src/own.rs` with `story:llvm-release-handle-temporaries`: it is merged
  after both.
