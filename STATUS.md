# Status

Standing of the current work items against the brief of 2026-11-02 (session `ignite`). Each
row is dated and names the command its result was read from.

| item | target | standing | date | source |
|---|---|---|---|---|
| recovery: telos scratch evidence | cited logs, scripts and drafts in the repository before the scratch directories expire | copied, 1,312 files, branch `docs/recover-telos-evidence` pushed; merge pending its gate | 2026-11-02 21:00 | `python3 -I copy_evidence.py`, `git ls-remote origin` |
| recovery: baseline source trees | every cited baseline tree on a remote ref | `bench/11b-recovered` (741 files identical to `s12-before`), tag `bench-12-close`, tag `bench-12b-checkpoint` pushed | 2026-11-02 20:58 | `git hash-object` per file, `git ls-remote origin` |
| recovery: slice-13 runtime tree | Codex's uncommitted work on a remote, tested | `841eee0` and `77962b4` (edits two Codex workers made until 21:09) on `feat/slice13-local-apply`, pushed; `task gate` exit 0 on that tree, 5 apply e2e tests pass | 2026-11-02 21:15 | `slot.sh gate-s13.log … task gate`, `git ls-remote origin` |
| recovery: branches | delete branches `main` fully holds | deleted `maintenance/commit-style`, `maintenance/semantic-commits`, `perf/budget-plan`, `perf/restore-budget` (local and remote); kept `bench/*`, `docs/slice13-contract`, `feat/slice13-local-apply`, `docs/recover-telos-evidence` | 2026-11-02 21:04 | `git merge-base --is-ancestor`, tree comparison per commit |
| recovery: stashes, worktree registry | nothing lost | no stashes; no registry record references `~/projects/telos` | 2026-11-02 21:05 | `git stash list`, `worktree status --json` |
| recovery: transcripts | edits that never reached a commit | 3,446 edit calls in 95 transcripts, 421 files: 287 last edits on main, 134 superseded, 0 lost; one earlier edit lost (slice 12 "gate, cold" row) and four "gate, cold" rows overwritten with the 12b row; all four restored in AGENTS.md | 2026-11-02 21:19 | transcript replay script, `git grep` over every commit, `git show 60aef38:AGENTS.md`, `afa82aa`, `d3a0d14` |
| slice 13 `ignite apply` | `docs/slice.md` 17.7 acceptance matrix, `task gate` green, merged, released | units U1 to U6 merged into `feat/slice13-local-apply`; `task gate` exit 0 at `d93f059`, 410 s, 746 cargo tests passed, 0 failed, 5 ignored, 160 inline tests per target | 2026-11-03 02:46Z | `slot.sh s13-gate.log … task gate` |
| slice 13 acceptance matrix | `docs/slice.md` 17.7, rows 1 to 15 | 46 passed, 0 failed, 0 ignored, 448.83 s | 2026-11-03 | `cargo test -p ignite --test main apply` |
| performance study | verdict A or B on the 11b budget, 30 min time box | verdict B, `docs/performance/13/verdict.md`; recorded as an exception, no baseline moved | 2026-11-03 | `docs/performance/13/verdict.md` |
| slice 14, 18.0 rows 1, 4, 5, 6 | Telnet inline suite on both targets, the pipe tests of 18.12 | 37 passed on `rust` and on `llvm`, exit 0 (28 before the 9 boundary probes of 18.12 "As landed (U5)"); `--list` shows the four pipe tests | 2026-11-03 07:09Z | `ignite test std/protocol/telnet.ignite --target all`, `--list` |
| slice 14, 18.0 rows 2, 7, 8 | HTTP inline suite on both targets, the pipe tests of 18.12 | 41 passed on `rust` and on `llvm`, exit 0 (27 before the 14 boundary probes); `--list` shows the three pipe tests | 2026-11-03 07:09Z | `ignite test std/protocol/http.ignite --target all`, `--list` |
| slice 14, 18.0 row 3 | the terminal transcript over a pipe | 4 passed on `rust` and on `llvm`, exit 0 | 2026-11-03 06:53Z | `ignite test examples/terminal/commands.ignite --target all` |
| slice 14, 18.0 rows 9 and 16 | TCP behaviour and goldens | 101 passed, 0 failed (`terminal::`, `todo::`, `plan::`, `eval::`, both targets); `terminal.rs` and `todo.rs` unchanged (diff exit 0); the model goldens differ only in `maximum_subnegotiation` and the todo server's `limits`, the explain goldens not at all | 2026-11-03 06:54Z | `cargo test -p ignite --test main -- terminal:: todo:: plan:: eval::`, `git diff --exit-code 340f782 -- crates/ignite/tests/terminal.rs crates/ignite/tests/todo.rs` |
| slice 14, 18.0 row 10 | Rust static dispatch | 1 passed | 2026-11-03 06:54Z | `cargo test -p ignite-rustgen --test main streams::` |
| slice 14, 18.0 row 11 | LLVM instances call their own entries | 2 passed | 2026-11-03 06:54Z | `cargo test -p ignite-llvm --test main streams::` |
| slice 14, 18.0 row 18 | `task gate` and the 18.18 measurements | not run | 2026-11-03 | |
