# Wave 1 of 6: apply-test-deadlines (2026-11-07)

Plan: [plan-2026-11-07.md](plan-2026-11-07.md)

## Plan

| wave | stories | objective | state |
|---|---|---|---|
| 1 | `story:apply-test-deadlines` | gates pass on the first run | merged (PR 88); story stays active: 2 of 3 gates started under load 15 |

- Approval: `approval-record:wave-apply-test-deadlines-2026-11-07` (option B; "then dispatch first /aep:wave").
- Check path: one story; scope typed by the coordinator from the story's test names (`crates/ignite/tests/<file>.rs` per harness name `<file>::<test>`, AGENTS.md).
- Commits authorised: the unit's commits on `fix/apply-test-deadlines`, its PR merged into `main` after a green `task gate`, store commits on `main`. No tag, no release.

## Units

| unit | branch | base | worktree | build dir | scratch | stage |
|---|---|---|---|---|---|---|
| `story:apply-test-deadlines` | `fix/apply-test-deadlines` | `0ce668d6` | `~/.local/state/worktree/trees/codewandler/ignite-lang/apply-test-deadlines` (managed id `apply-test-deadlines`) | `<worktree>/target` | `~/.cache/scratch/e18a99fc-f99c-4987-93c9-e5de2080b61a/unit-apply-test-deadlines` | merged, PR 88, `7b3afbd1`; head `7d095a9f` |

- Store tree: managed id `wave-store-apply-test-deadlines`.
- Pre-flight 2026-11-07 10:16 CET: `/` 165 G free of 848 G; load 4.29.

## Close

- Adversary: pass 1 4 findings (3 fixed, 1 no-op); pass 2 2 new, 0 carried (2 fixed). The coordinator read the round-2 diff: no assertion dropped or loosened.
- Gate on `7d095a9f`, 3 runs, every step exit 0: 1319 cargo passed, 0 failed, 11 ignored; 309 inline; `cargo test` 1687, 1440, 1488 s; start loads 15.24, 9.87, 6.55.
- Story not moved to `implemented`: its row "3 consecutive gates at load over 15" holds for run 1 only.
- Scope learned: `crates/ignite/tests/todo.rs` and `docs/slice.md` added; `apply_admission.rs`, `apply_way_out.rs` and `run.rs` changed only in round 1 and 2 (constants, the `gave_up` wait).
- Agent tokens: implementor 186,236, 209,937, 228,005 (3 rounds); adversary 112,562 and 87,046; story scopers 51,661 to 60,106 each (6).
