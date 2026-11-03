You work in /home/timo/projects/telos, a Rust compiler for a language called Telos, with two targets (Rust and LLVM), a std library written in the language, and a substrate in Rust. Read `AGENTS.md` first.

The language is renamed from Telos to Ignite (Timo's order). Your task: replace every occurrence of the name in the tree and leave the gate green. You are the only agent working in the tree.

## Scope

Every tracked and untracked file outside `target/` and `.git/` (`git ls-files -co --exclude-standard`):

| from | to |
|---|---|
| `telos` | `ignite` |
| `Telos` | `Ignite` |
| `TELOS` | `IGNITE` |
| any other casing | the same casing of `ignite` |

This covers:
- file contents: Rust identifiers, crate names, C ABI symbols (`telos_rt_*` → `ignite_rt_*`), env vars, CLI name (`telos` → `ignite`), Taskfile tasks and paths, docs, fixtures, `.expected` files, goldens, doc comments;
- paths: `crates/telos-*` → `crates/ignite-*`; every `*.telos` file → `*.ignite`, and the compiler must accept the `.ignite` extension in place of `.telos` (module loading, `module("…")`, std lookup, `telos test` file discovery); any other file or directory with the name in it;
- `docs/design.md`, `docs/vision.md` and the root `PLAN.md` (contents only; their file names stay);
- the name as a word inside prose, including the Greek-word sense, and as a substring of a longer word.

Exceptions:
- The repository directory `/home/timo/projects/telos` keeps its name; do not rename it. The repository will be published, so no file may contain that absolute path: replace it with a path relative to the file (for example `docs/audit/2026-11-01/io/iotest/Cargo.toml` gets `path = "../../../../../crates/ignite-io"`) or, in prose, with a repository-relative path (`docs/research/ess.md`, `docs/research/lean.md`). Build iotest afterwards with its own `--target-dir` under the scratch directory to show the relative path resolves.
- Paths under `/home/timo/.cache/scratch/` inside tracked files: list them in your report and leave them.
- Do not touch `target/` or `.git/`. Do not delete any `target/` directory.
- `CLAUDE.md` is a pointer to `AGENTS.md`; leave it as it is unless it contains the name.
- `README.md` will be rewritten by the coordinator after you; apply the rename to it as to any other file.

Use `mv` for renames (no git command that changes state). Let cargo regenerate `Cargo.lock`. Regenerate goldens with the repository's bless mechanism, not by hand, and check that each golden diff is only the rename.

## One flaky test to fix as well

`crates/telos-rt/tests/abi.rs` `release_closes_the_socket_and_invalidates_the_handle` failed once in six runs (P4's warm gate, 2026-11-02): line 223 re-binds the released listener's port and got `Address already in use (os error 98)`, because a test running in parallel can take that port as its ephemeral port after the release. Make the test prove the release without depending on the exact port staying free (for example: check that the listener's file descriptor is closed, or retry the bind only while the error is EADDRINUSE and the port belongs to another socket). Show it green in 30 consecutive runs of the whole `telos-rt` test binary (renamed), run in the background.

## Done when

- `git ls-files -co --exclude-standard | xargs grep -il telos` lists nothing except the files of the exceptions above, and no path in that list contains `telos` in any casing.
- `cargo fmt --check` is clean.
- `cargo clippy --workspace --all-targets -- -D warnings` is clean.
- `cargo test --workspace --no-fail-fast` is green, with the same counts as before the rename (record the counts of a run before you start, from the same target dir).
- `task test-telos` (renamed) is green on both targets, with the same inline-test counts as before.
- `task compile`, `task compile-llvm`, `task compile-todo` and `task compile-todo-llvm` (or their renamed forms) succeed, and the built `terminald`, `tn` and `todod` run.
- `task gate` (renamed if needed) is green.

## Rules

- Use `CARGO_TARGET_DIR=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/target-ign` for direct cargo calls.
- Every cargo call sets `CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_INCREMENTAL=0`. Release builds set `CARGO_PROFILE_RELEASE_DEBUG=0`.
- Prefix test runs with `ulimit -c 0;`.
- Run `df -h /` before your first build; do not build with under 20G free.
- Run one cargo or task run at a time. Anything longer than about 30 s goes to the background; wait on its own PID or on a final `EXIT=` line you write into its output file yourself. Do not stack runs.
- The 100,000-term build-and-run tests are `#[ignore]`; do not run them.
- No git command that changes state: no add, commit, mv, checkout, restore, stash or reset. No commits.
- Never call the GitLab API, with `curl` or anything else, using `$GITLAB_PERSONAL_TOKEN` or any other personal token of Timo's.
- No work files in /tmp or /dev/shm; use /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/. No heredocs feeding stdin readers, no `python3 -c`; write scripts with the Write tool and run them with `python3 -I`.
- No comments in `Cargo.toml`, `Taskfile.yml` or any other config file.
- A `terminald` on port 2323 belongs to Timo; leave it alone. Use free ports in tests.
- Kill your own processes by explicit PID before you finish.
- On an HTTP 429 or usage-limit error from a tool, continue; do not reduce scope.

## Final report, at most 12 lines

- files changed and paths renamed (counts);
- the remaining `telos` hits and why each is left;
- cargo test counts before and after; inline-test counts before and after;
- the gate commands and their exit codes;
- anything the rename broke and how you fixed it.
