You work in /home/timo/projects/telos, a Rust compiler for the Telos language with two targets, Rust and LLVM. Read `AGENTS.md`, `docs/slice.md` section 12 (tests) and `crates/telos/src/test_build.rs` first.

You are S9B. A second agent, S9C, runs at the same time on the LLVM target: overflow checks, debug metadata, the triple and the opt level. S9C owns `crates/telos-llvm/**`, `crates/telos-rt/**`, `crates/telos/src/llvm_build.rs`, `docs/llvm.md`, and the `[profile.release]` line of the Cargo.toml that rustgen generates for a program. You own `crates/telos/src/test_build.rs`, the `tests/` layout of every crate and its `[[test]]` entries, `AGENTS.md` and the rest of `crates/telos-rustgen`. If you need a change in `crates/telos-llvm`, such as a per-package symbol prefix or the test table, ask S9C by SendMessage instead of editing its files. Both of you use one shared tree, so goldens and gate runs may fail on the other agent's work in progress. If that happens, wait and re-run; do not fix the other agent's files. S9C uses target-h, and you use target-k. The task is slice 9b: test build time. It comes from Timo, by way of the steering session.

Timo's principle: a main goal of Telos is compile time kept to a minimum compared to Rust, with Rust-like safety and good error reporting, all serving agentic development. Rust builds one test binary per test file, each its own compile and link, and that was always the problem. Telos puts everything into a single test binary by default, on every backend, for the full gate.

## 1. `telos test`: one test executable per target

- Today `test_build.rs` builds one executable per program plus one for std. That is 3 per target in `task test-telos`.
- Make the default one test executable per target for all the files given: std modules and every program.
  - Programs need package-level namespacing inside the shared executable. On Rust, use a module per package. On LLVM, use a symbol prefix per package.
  - Test names in the output stay as they are today: `package/test name`.
- Keep `--separate` as an opt-in for the old layout only if it costs nothing; otherwise remove it.
- Fix and keep the regression test for the stale test binary when two files share one `--target-dir`, if N4 added one.
- Acceptance:
  - `task test-telos` builds and links 1 executable per target;
  - every inline test still passes (72 per target before slice 9's rewrite; use the count N4 reports);
  - the wall time of `task test-telos` before and after is recorded.

## 2. The compiler's own cargo tests: one integration binary per crate

- Today there are 39 integration-test files across 8 crates, each its own binary: telos 10, telos-types 8, telos-check 4, telos-rustgen 4, telos-syntax 4, telos-io 3, telos-llvm 3, telos-rt 3. Count them again yourself first.
- Consolidate each crate into one integration-test binary: `tests/main.rs` declares the existing files as `mod`. Set `autotests = false` and one `[[test]]` in each Cargo.toml if needed; no comments in Cargo.toml.
- Unit tests stay where they are.
- Fixture paths, `env!("CARGO_BIN_EXE_…")` uses and `#[ignore]` tests must keep working.
- Acceptance:
  - `cargo test --workspace --no-run` produces at most one integration binary per crate;
  - the test count (passed plus ignored) is unchanged, before vs after;
  - the wall time of `task gate` is recorded before and after, twice each: cold with `SCCACHE_RECACHE=1`, and warm after `touch crates/telos-check/src/lib.rs`.

## 3. Record the budget

- Add a "gate time" line to `AGENTS.md` with the date, the command and the numbers from 1 and 2, so the next slice sees the budget.
- Baseline from the steering session (18:35, todo example, cold, 20 cores):
  - Rust target: 3.0 s dev, 3.9 s release;
  - LLVM target: 0.8 s dev, 0.9 s release (emit, clang and link);
  - `libtelos_rt.a`, built once: 3.0 s dev, 4.7 s release.
- Update `docs/slice.md` section 12 where it describes one test executable per file.

Second baseline, measured by the steering session at 18:45 in a copy of the tree with the compiler already built. The command `cargo test --workspace --no-run`:
- produces 49 test executables totalling 699 MB;
- takes 7.4 s, with sccache hits for dependencies (a true cold build was not measured);
- takes 0.1 s warm;
- takes 4.2 s after `touch crates/telos-check/src/lib.rs`.

Repeat these measurements before and after your change, with the same commands, and record them in `AGENTS.md`:
- the executable count;
- the total size (`du`);
- the time with dependencies cached;
- the warm time;
- the time after the touch.

The principle is now invariant 9 in `docs/vision.md` section 3, and 9b is in its 4.1 table.

## Gate

- `task gate` is green.
- `task compile`, `task compile-llvm`, `task compile-todo` and `task compile-todo-llvm` succeed.

## Rules

- Use `CARGO_TARGET_DIR=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/target-k` for direct cargo calls.
- Every cargo call sets `CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_INCREMENTAL=0`.
- Prefix test runs with `ulimit -c 0;`.
- Run one cargo or task run at a time. Anything longer than about 30 s goes to the background; wait on its own PID or on a final `EXIT=` line that you write into its output file yourself.
- Kill your own processes by explicit PID before you finish.
- The 100,000-term chain tests are `#[ignore]`; do not run them.
- The rest of the rules are in `/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/rules.md`; read and follow them.
- No git command that changes state: no add, commit, checkout, restore, stash or reset.
- Never call the GitLab API, with `curl` or anything else, using `$GITLAB_PERSONAL_TOKEN` or any other personal token of Timo's.
- A `terminald` on port 2323 belongs to Timo; leave it alone.
- No work files in /tmp; use the scratch directory.

## Final report

At most 12 lines:
- executables per target before and after;
- integration binaries per crate before and after;
- test counts before and after;
- the 3 wall times (test-telos, gate cold, gate warm) before and after;
- deviations.
