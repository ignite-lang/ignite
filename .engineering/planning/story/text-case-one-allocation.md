---
format: aep.planning-md/3
id: story:text-case-one-allocation
kind: story
status: draft
title: to_lower and to_upper allocate once; Bytes set guard tests
scope:
- confidence: inferred
  path: crates/ignite-llvm/src/ir.rs
- confidence: cited
  path: crates/ignite-llvm/src/lower.rs
- confidence: inferred
  path: crates/ignite-llvm/tests/golden
- confidence: inferred
  path: crates/ignite-rt/src/lib.rs
- confidence: inferred
  path: crates/ignite-rt/src/text.rs
- confidence: cited
  path: crates/ignite-rustgen/src/lib.rs
- confidence: inferred
  path: crates/ignite-rustgen/tests/golden
- confidence: inferred
  path: crates/ignite-types/src/lib.rs
- confidence: inferred
  path: crates/ignite-types/src/moves.rs
- confidence: inferred
  path: crates/ignite/src/apply/plan.rs
- confidence: cited
  path: crates/ignite/tests/fixtures/eval_equivalence.ignite
- confidence: cited
  path: crates/ignite/tests/fixtures/tests/bytes_set.ignite
- confidence: cited
  path: crates/ignite/tests/ignite_test.rs
- confidence: inferred
  path: docs/slice.md
- confidence: inferred
  path: std/text.ignite
revision: 16
---
# Story: text-case-one-allocation

## Outcome

`to_lower` and `to_upper` allocate once per call on both targets, and the Bytes `set` guard tests of the PR 82 adversary review are on main.

## Context

- Adversary review of PR 82 (2026-11-07) found no defect; branch `test/adv-bytes-set` at `a3ebc917` adds guard tests (fixture `crates/ignite/tests/fixtures/tests/bytes_set.ignite`, 8 passing and 5 failing on purpose with pinned messages; `bytes_case_views` in `eval_equivalence.ignite`; `ignite_test::bytes_set_on_both_targets`), all green on main.
- On the Rust target `to_lower` (`std/text.ignite:602`) still allocates twice: `text.clone().into_bytes()`, then `String::from_utf8_lossy(&out).into_owned()`; on LLVM `to_text` goes through `text_from_bytes_lossy` (inferred, `crates/ignite-llvm/src/lower.rs:4312`).

## Acceptance

- The tests of `test/adv-bytes-set` are on main and pass.
- A `Bytes` that came from a `Text` and was changed only in ASCII bytes converts back without a second copy or re-validation on both targets, or the story records why not; `task gate` green.
