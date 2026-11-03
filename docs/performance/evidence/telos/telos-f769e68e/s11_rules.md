Rules for every slice-11 agent (Timo's, non-negotiable):

- No git commits, and no git command that changes state: no add, commit, mv, checkout, restore, reset or stash. Uncommitted changes in the tree belong to other agents.
- Every cargo command runs with `CARGO_TARGET_DIR=<your target dir> CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only`. Release builds also set `CARGO_PROFILE_RELEASE_DEBUG=0`. Prefix test runs with `ulimit -c 0;`.
- Run `df -h /` before your first build and before large builds; stop and report if under 20G is free. A disk watch may stop cargo and rustc under the scratch directory below 20G free; if your build dies unexpectedly, run `df -h /` first.
- Run one cargo or task run at a time. Anything likely to run longer than about 30 s goes to the background; wait on its own PID or on a final `EXIT=` line you write into its output file yourself. Do not stack runs.
- The 100,000-term build-and-run tests are `#[ignore]`; do not run them.
- No work files in /tmp or /dev/shm. Use `/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/` with unique file names.
- No heredocs into stdin readers, and no `python3 -c`. Write scripts with the Write tool, run them by path (`python3 -I <script>`), delete them when done.
- Never delete a `target/` directory by hand, yours or anybody else's.
- Never call the GitLab API, with `curl` or anything else, using `$GITLAB_PERSONAL_TOKEN` or any other personal token of Timo's.
- No comments in `Cargo.toml`, `Taskfile.yml` or other config files. Do not add a dependency without a line in your report saying what it replaces.
- Generated Rust and LLVM IR are artifacts: never edit them by hand. Goldens are re-blessed with `IGNITE_BLESS=1`, and every golden diff is explained in your report.
- A `terminald` on port 2323 belongs to Timo; leave it alone. Tests use free ports.
- Kill every process you started, by explicit PID, before you finish.
- Do not edit files owned by another agent. If you need a change there, ask its owner by SendMessage (the coordinator gives you the ids) or say so in your report.
- If clippy or fmt fails only in another agent's in-progress files, report it and do not fix it. The tree is shared, so a run may fail on another agent's work in progress; wait and re-run.
- On an HTTP 429 or usage-limit error from a tool, continue; do not reduce scope.
- Reports go to the coordinator, at most the number of lines your brief gives: numbers, file paths and commands, no prose.
