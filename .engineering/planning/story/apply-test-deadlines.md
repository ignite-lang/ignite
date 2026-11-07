---
format: aep.planning-md/3
id: story:apply-test-deadlines
kind: story
status: active
title: Live apply and rollout tests pass a loaded gate on the first run
scope:
- confidence: cited
  path: crates/ignite/tests/apply_admission.rs
- confidence: cited
  path: crates/ignite/tests/apply_way_out.rs
- confidence: cited
  path: crates/ignite/tests/common/apply.rs
- confidence: cited
  path: crates/ignite/tests/common/mod.rs
- confidence: cited
  path: crates/ignite/tests/drain.rs
- confidence: cited
  path: crates/ignite/tests/run.rs
- confidence: cited
  path: crates/ignite/tests/slice16.rs
- confidence: cited
  path: crates/ignite/tests/slice18.rs
- confidence: cited
  path: crates/ignite/tests/terminal.rs
revision: 12
transitions:
- {from: "draft", to: "proposed", at: "2026-11-07T09:16:05Z", actor: "human:timo", revision: 11}
- {from: "proposed", to: "active", at: "2026-11-07T09:16:05Z", actor: "human:timo", revision: 12}
---
# Story: apply-test-deadlines

## Outcome

The live apply and rollout tests pass a loaded `task gate` on the first run: their deadlines have headroom over the time the step takes alone.

## Context

- Gates of 2026-11-07 failed only on these, each passing 5/5 alone:
  - `apply_admission::changed_codegen_revision_with_an_unchanged_model_replaces_the_hosted_server` (Rust): replace over 120 s; alone 80-121 s (`crates/ignite/tests/apply_admission.rs:164-172`);
  - `apply_way_out::llvm_recover_ends_a_live_instance`: apply-gives-up over 120 s; alone 97.75-100.86 s;
  - `slice18::llvm::todo_survives_sigkill_and_a_rollout` (first-apply over 120 s), `slice18::llvm::a_plan_saved_before_a_source_change_is_refused` (host did not end), `slice18::llvm::the_65537th_todo_survives_a_restart`;
  - `drain::rollout_drains_the_old_process_and_starts_the_new_{rust,llvm}`, `terminal::llvm::a_task_that_cannot_start_closes_only_its_connection`, `run::exhausted_restarts_give_up_and_stop_exits_1` (60 s setup on the cargo build lock), `slice16::llvm_state_threading_is_linear`.
- The release gate of v0.12.0 needed 3 runs; PR 72's and PR 85's 2-5.

## Acceptance

- Each test above: its alone time measured (5 runs) and its deadline at least 2x the slowest, or the step it waits on made faster; the deadlines are named in one place per file.
- 3 consecutive `task gate` runs on one head at load over 15 pass with no rerun.
