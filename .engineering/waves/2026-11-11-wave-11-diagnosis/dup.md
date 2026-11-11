# Diagnosis: row 1.3, rules implemented more than once

Read-only diagnosis of 2026-11-11 on `wave/10-tls` at `98c8de54` against the tag `v0.15.0`.
Nothing was built or run; counts come from the counter's rule applied with `git grep` and awk.
Equivalence claims are inferred and say so.

## The counter (`crates/ignite/tests/quality.rs`)

- `static_metrics_hold_the_bar` calls `static_metrics`, which reads every `*.rs` file under
  `crates/{ignite-rustgen,ignite-llvm,ignite-eval}/src` as raw text: comments, doc comments
  and `#[cfg(test)]` modules count; `tests/` is not read.
- patterns: every occurrence of the text `PatternKind::`.
- builtins: distinct names after `Builtin::` that start uppercase. Names under
  `use Builtin::*` are invisible to it. This part of the counter is unchanged since v0.15.0.

## Values

| | rust/patterns | rust/builtins | llvm/patterns | llvm/builtins | eval/patterns | eval/builtins |
|---|---|---|---|---|---|---|
| v0.15.0 | 83 | 54 | 30 | 14 | 13 | 133 |
| `98c8de54` | 83 | 57 | 46 | 13 | 13 | 149 |

Both rows match `quality/history.jsonl`.

## What was added

- eval +16: ByteToU32, IntToU32, IntWrapByte, IntWrapU32, U32ToByte, U32ToInt, U32ToText,
  U32WrapByte, U32RotateLeft, U32RotateRight (`c1e22da2`); RuntimeCensus, RuntimeSettle
  (`b6b85104`); TcpListenerBindKept (`fb77af0b`); TcpStreamLocal (`22881cd6`); TlsClient,
  TlsStreamAlpn (`0450ce20`).
- rust +3: TcpStreamLocal (`22881cd6`), TlsStreamAlpn (`0450ce20`), U32ToText (`c1e22da2`).
- llvm/patterns +16, all from `380204de` ("release every handle with its last owner"):
  `own.rs` `owned` (3 mentions, 2 at v0.15.0), `fresh_binding` (3, new), `collect_names`
  behind `pattern_names` (12, new). Unchanged: `lower.rs` `pattern` 14, `rest_list_pattern` 3,
  `region.rs` `pattern_binds` 11.

## What each does, and where the same rule already lives

- eval, the ten U32/Byte/Int conversions (`builtin_value`): they evaluate the builtin. Every
  backend implements these; there is no shared rule to reuse.
- eval RuntimeCensus, RuntimeSettle, TcpListenerBindKept, TlsClient: entries of the table
  `is_static`; each is then refused by the fallback of `builtin_static`. The same "is a
  static" rule is in `ignite-types/src/builtins.rs` `static_fn`, rustgen `BUILTIN_STATICS`
  and `moves.rs` `static_receiver`.
- eval TcpStreamLocal, TlsStreamAlpn: matched only to refuse, in the arm of `builtin_value`
  that lists 55 method builtins as having no value at compile time. The final arm would catch
  them with a different message.
- rust U32ToText: in three lists that mean "the scalar `to_text` is Rust's Display"
  (`fresh_text`, `format_part`, `method`). The same four names are in eval `show`, llvm `show`
  and the typer's `show`. There is no `impl Builtin` anywhere.
- rust TcpStreamLocal: lowers the builtin, a copy of the TcpStreamPeer arm except for
  `.local()`. rust TlsStreamAlpn: a same-name passthrough in a list with the seven
  PipeStream names.
- llvm `collect_names`: "the names a pattern binds". The same walk exists in llvm
  `region.rs` `pattern_binds`, rustgen `pattern_binds` and `pattern_bindings`,
  `ignite-types/src/moves.rs` `unbind` and `ignite-check` `first_binding`. `impl Pattern` in
  `ignite-syntax` has only `new`.
- llvm `owned` and `fresh_binding`: the same shallow rule (one binding or a tuple of
  bindings) written twice in `own.rs`.

## Candidate changes

- L1 (llvm/patterns 46 → 23): `Pattern::bindings` (source order, every `|` alternative
  contributes) and `Pattern::binds` on `impl Pattern` in `ignite-syntax`; `own.rs`
  `pattern_names` and `region.rs` `pattern_binds` delegate, `collect_names` goes. Extended to
  rustgen `pattern_binds` and `pattern_bindings`: rust/patterns 83 → 65. Risk: the rustgen walk
  descends into `Range` bounds and the llvm ones do not; bounds bind nothing (inferred), so
  generated code should not change (inferred). Held by the llvm golden tests, the `leak::`
  soaks and the differential corpus.
- L2 (optional, −3): one shallow-bindings helper for `owned` and `fresh_binding`.
- R1 (rust/builtins 57 → 53): `Builtin::is_scalar_to_text` (IntToTextMethod, ByteToText,
  U32ToText, BoolToText) in `ignite-types`, used by rustgen `fresh_text`, `format_part` and
  `method`, and by eval `show`, llvm `show` and the typer's `show`. No generated-code change
  if the set is those four. Held by the rustgen golden tests and the differential tests.
- R2 (−2): one rule for TcpStreamPeer and TcpStreamLocal on the result type. R3 (−8): the
  stream passthrough list decided by receiver type; risks a generated-code change.
- E1 (eval/builtins 149 → 94; 124 on the tree of the evaluator's model): delete the refusal
  arm of `builtin_value` and let the last arm say that the builtin has no value at compile
  time. No reason in `eval_cannot_run` names a method builtin (all name a static), so the list
  is unchanged (reasoned from the list, not run).
- E2 (fallback, 149 → 122): `Builtin::is_static` in `ignite-types`, used by eval, rustgen and
  `moves.rs`. Every baseline refusal is a static, so a difference in the set changes reasons.

Taken together L1 + R1 + E1 give llvm/patterns 23, rust/builtins 53, eval/builtins 94 (124
with the evaluator's model merged); the other cells stay at or below v0.15.0.

## Side finding

The counter cannot see names under `use Builtin::*`. The llvm crate names 149 builtin variants
as bare words today, 133 at v0.15.0 (an upper bound from a word match). `380204de` added
another glob-imported list (`own.rs` `creates`, 20 names). So llvm/builtins going from 14 to
13 does not show that the llvm crate names fewer builtins.

## Not checked

- L1's equivalence (ranges, duplicates from `|`, order) and the claim that no baseline unit
  reaches the deleted arm are reasoned from the code.
- Whether E2's set equals `static_fn` plus the generic statics.
