# Wave 1 of 2: apply-test-deadlines (2026-11-07)

## Plan

| wave | stories | objective | state |
|---|---|---|---|
| 1 | `story:apply-test-deadlines` | gates pass on the first run | dispatched |
| 2 | `story:spawn-moves-adversary-findings`, `story:rustgen-reserved-names`, `story:http-chunked-adversary-findings`, `story:store-adversary-findings`, `story:store-api-gaps`, `story:text-case-one-allocation` (option A, inferred from the draft list; still draft) | the six drafts | not proposed |

- Approval: `approval-record:wave-apply-test-deadlines-2026-11-07` (option B; "then dispatch first /aep:wave").
- Check path: one story; scope typed by the coordinator from the story's test names (`crates/ignite/tests/<file>.rs` per harness name `<file>::<test>`, AGENTS.md).
- Commits authorised: the unit's commits on `fix/apply-test-deadlines`, its PR merged into `main` after a green `task gate`, store commits on `main`. No tag, no release.

## Units

| unit | branch | base | worktree | build dir | scratch | stage |
|---|---|---|---|---|---|---|
| `story:apply-test-deadlines` | `fix/apply-test-deadlines` | `0ce668d6` | `~/.local/state/worktree/trees/codewandler/ignite-lang/apply-test-deadlines` (managed id `apply-test-deadlines`) | `<worktree>/target` | `~/.cache/scratch/e18a99fc-f99c-4987-93c9-e5de2080b61a/unit-apply-test-deadlines` | implementor dispatched |

- Store tree: managed id `wave-store-apply-test-deadlines`.
- Pre-flight 2026-11-07 10:16 CET: `/` 165 G free of 848 G; load 4.29.
