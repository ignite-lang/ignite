You work in /home/timo/projects/telos, a Rust compiler for the Telos language with two targets, Rust and LLVM. Read `AGENTS.md`, `docs/llvm.md`, `docs/slice.md` section 13 and `docs/vision.md` sections 4.1 (row 9c) and 7 (T6) first.

You are S9C, slice 9c: the LLVM target's disadvantages. It comes from Timo, by way of the steering session. Another agent, S9B, runs at the same time on test build time.

## Ownership

| who | owns |
|---|---|
| you | `crates/telos-llvm/**`, `crates/telos-rt/**`, `crates/telos/src/llvm_build.rs`, `docs/llvm.md`, and the one release-profile line of the Cargo.toml that `crates/telos-rustgen` generates for a program (item 1) |
| S9B | `crates/telos/src/test_build.rs`, the `tests/` layout of every crate, the Cargo.toml `[[test]]` entries, `AGENTS.md` and everything else in `crates/telos-rustgen` |

S9B needs one test executable per target, with a symbol prefix per package on LLVM. If it asks you by SendMessage for a change in `crates/telos-llvm`, make it, so that only one agent edits those files. Tell S9B before you touch anything outside your files.

## Scope

1. **T6: Int arithmetic overflow is a failure** on both targets in every profile. It ends the task like a failed assert.
   - LLVM: use `llvm.sadd/ssub/smul.with.overflow` for `+`, `-` and `*`, with a branch to the failure path. Use the checked forms of `/` and `%`, which fail on division by zero and on MIN / -1.
   - Rust: set `overflow-checks = true` in the generated crate's `[profile.release]`. Make sure `/` and `%` give the same report as on LLVM.
   - Acceptance: one inline test per operator overflows (`expect fail` style, as in the slice-8 runner's failure tests). It fails with the same report text on both targets, in dev and in release.
   - Byte arithmetic follows the same rule, if `docs/slice.md` defines Byte `+`; check and state what you did.
2. **Debug metadata** in the emitted IR, mapping to the Telos source rather than to the IR.
   - Emit `DICompileUnit`, one `DIFile` per module, one `DISubprogram` per fn, and `!dbg` on each instruction with the Telos line and column.
   - Pass `-g` to clang.
   - Acceptance, headless:
     - `gdb -batch` with a breakpoint on a line of `examples/todo/todo.telos` hits in the LLVM todod. Drive it with a request on a free port, not 8080 and not 2323.
     - `perf record` / `perf report` shows Telos fn names and lines. If perf is not permitted, say so and show `llvm-symbolizer` or `addr2line` output instead.
   - Record the size change of todod.
3. **Pin the target triple and datalayout** in every emitted module.
   - Take them from `clang -print-target-triple` and a probe module at build time, or from a `--triple` flag on `telos build` that is also passed to clang.
   - Check the runtime archive's triple against it.
   - Acceptance: the emitted IR starts with the triple, and a mismatched `--runtime` fails with a clear error before clang runs. Add tests for both.
4. **Dev optimisation level.**
   - Measure compile time and inline-test wall time of `task test-telos` on the LLVM target at -O0 and at -O1.
   - Pick -O0 unless the test suite runs more than 2x slower.
   - Record both numbers in `docs/llvm.md`.
   - Note: N3 made the chain-append helpers `noinline` because clang -O1 ran for over 11 minutes otherwise; keep that.
5. **Not in scope:** coroutines (slice 5), T1 unwind (slice 10), and the C ABI layer.

## Gate

- `task gate` is green on both targets.
- `docs/llvm.md` is updated wherever the memory model, the ABI or the build changed.
- `task compile-llvm` and `task compile-todo-llvm` succeed.
- Goldens: every `.ll` golden changes because of the triple and the debug metadata. Regenerate them and explain the diff in one line per kind of change.

## Rules

- Use `CARGO_TARGET_DIR=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/target-h` for direct cargo calls. S9B uses target-k.
- Every cargo call sets `CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_INCREMENTAL=0`. Release builds set `CARGO_PROFILE_RELEASE_DEBUG=0`.
- Prefix test runs with `ulimit -c 0;`.
- Run one cargo or task run at a time. Anything longer than about 30 s goes to the background; wait on its own PID or on a final `EXIT=` line you write into its output file.
- Kill your own processes (servers, gdb, perf) by explicit PID before you finish.
- The 100,000-term chain tests are `#[ignore]`; do not run them.
- The rest of the rules are in `/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/rules.md`; read and follow them.
- No git command that changes state: no add, commit, checkout, restore, stash or reset.
- Never call the GitLab API, with `curl` or anything else, using `$GITLAB_PERSONAL_TOKEN` or any other personal token of Timo's.
- A `terminald` on port 2323 belongs to Timo; leave it alone.
- No work files in /tmp; use the scratch directory.

## Final report

At most 12 lines:
- each item done or not, with its acceptance evidence (the command and its result);
- the -O0 and -O1 numbers;
- the todod size before and after;
- the golden diff kinds;
- deviations.
