# Status

Standing of the current work items against the brief of 2026-11-02 (session `ignite`). Each
row is dated and names the command its result was read from.

| item | target | standing | date | source |
|---|---|---|---|---|
| recovery: telos scratch evidence | cited logs, scripts and drafts in the repository before the scratch directories expire | copied, 1,312 files, branch `docs/recover-telos-evidence` pushed; merge pending its gate | 2026-11-02 21:00 | `python3 -I copy_evidence.py`, `git ls-remote origin` |
| recovery: baseline source trees | every cited baseline tree on a remote ref | `bench/11b-recovered` (741 files identical to `s12-before`), `bench/12-close-recovered`, `bench/12b-checkpoint` pushed | 2026-11-02 20:58 | `git hash-object` per file, `git ls-remote origin` |
| recovery: slice-13 runtime tree | Codex's uncommitted work on a remote, tested | `841eee0` and `77962b4` (edits two Codex workers made until 21:09) on `feat/slice13-local-apply`, pushed; `task gate` exit 0 on that tree, 5 apply e2e tests pass | 2026-11-02 21:15 | `slot.sh gate-s13.log … task gate`, `git ls-remote origin` |
| recovery: branches | delete branches `main` fully holds | deleted `maintenance/commit-style`, `maintenance/semantic-commits`, `perf/budget-plan`, `perf/restore-budget` (local and remote); kept `bench/*`, `docs/slice13-contract`, `feat/slice13-local-apply`, `docs/recover-telos-evidence` | 2026-11-02 21:04 | `git merge-base --is-ancestor`, tree comparison per commit |
| recovery: stashes, worktree registry | nothing lost | no stashes; no registry record references `~/projects/telos` | 2026-11-02 21:05 | `git stash list`, `worktree status --json` |
| recovery: transcripts | edits that never reached a commit | 3,446 edit calls in 95 transcripts, 421 files: 287 last edits on main, 134 superseded, 0 lost; one earlier edit lost (slice 12 "gate, cold" row) and four "gate, cold" rows overwritten with the 12b row; all four restored in AGENTS.md | 2026-11-02 21:19 | transcript replay script, `git grep` over every commit, `git show 60aef38:AGENTS.md`, `afa82aa`, `d3a0d14` |
| slice 13 `ignite apply` | `docs/slice.md` 17.7 acceptance matrix, `task gate` green, merged, released | contract PR #1 open; unit U1 (split host.rs) running; units U2 to U6 planned | 2026-11-02 21:19 | `gh pr view 1` |
| performance study | verdict A or B on the 11b budget, 30 min time box | not started (runs after slice 13) | 2026-11-02 21:05 | |
