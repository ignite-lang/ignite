# Wave 2 of 6: spawn-moves-adversary-findings (2026-11-07)

Plan: [plan-2026-11-07.md](plan-2026-11-07.md)

- Approval: `approval-record:plan-2026-11-07-waves-2-to-6` ("A accept all").
- N = 1: the story shares a scope path with every other accepted story of the plan.
- Commits authorised: the unit's commits on `fix/spawn-moves-adversary-findings`, its PR merged into `main` after a green `task gate`, store commits on `main`. No tag, no release.
- Agent: `aep:implementor`, then `aep:adversary`.

## Units

| unit | branch | base | worktree | build dir | scratch | stage |
|---|---|---|---|---|---|---|
| `story:spawn-moves-adversary-findings` | `fix/spawn-moves-adversary-findings` | `f4261010` | `~/.local/state/worktree/trees/codewandler/ignite-lang/spawn-moves` (managed id `spawn-moves`) | `<worktree>/target` | `~/.cache/scratch/e18a99fc-f99c-4987-93c9-e5de2080b61a/unit-spawn-moves` | merged, PR 89, `44ffc015`; head `ceea7996` |

- Pre-flight 2026-11-07: `/` 121 G free of 848 G; load 16.68; worktrees: the store tree and `release-v0-12-0` (not this wave's).

## Close

- Adversary: pass 1 2 findings (adv1 inner shadowing, adv3 branch with its own owner mark; both fixed); pass 2 1 new, 0 carried (later match arm, for loop or closure parameter of the same name; fixed). The coordinator read the round-2 diff: no assertion dropped.
- Gate on `ceea7996`, every step exit 0: 1327 cargo passed, 0 failed, 11 ignored; 309 inline per target; `cargo test` 1790 s; start load 6.62.
- Story moved to `implemented`.
- Scope learned: `crates/ignite-rt/src/net.rs` (owner-list tombstones), `crates/ignite-llvm/src/region.rs`, `docs/llvm.md`, `nested_spawn_arms.ignite`; `crates/ignite-types/src/moves.rs` untouched.
- Agent tokens: implementor 123,683, 175,497, 190,918 (3 rounds); adversary 127,271 and 125,516.
