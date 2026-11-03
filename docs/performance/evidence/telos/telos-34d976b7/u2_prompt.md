You work in /home/timo/projects/telos, a Rust compiler for the Telos language with two targets:
- Rust: generated crates built by cargo.
- LLVM: IR built by clang and linked with `libtelos_rt.a`, the C ABI adapter over the `telos-io` substrate.

Read these first:
- `AGENTS.md`;
- `docs/llvm.md`;
- the approved slice-8 plan, `/home/timo/.claude/plans/deep-marinating-snowflake.md`;
- `docs/slice.md` section 12, the slice-8 contract U1 just landed: 12.4 typed API, 12.5 assert and `?` failure format, 12.7 runner ABI and output, 12.9 the list of placeholders you replace.

You are U2: the slice-8 substrate, backends and driver. You own `crates/telos-io/**`, `crates/telos-rt/**`, `crates/telos-rustgen/**`, `crates/telos-llvm/**` and `crates/telos/**`. U1 owns `telos-syntax`, `telos-check` and `telos-types`, and is still doing parser work, diagnostics and fixtures. Don't edit those crates. If you need something from them, message U1 (agent id `aeae4669972d8ba84`, via SendMessage) and continue with other work.

Deliver:
1. **Substrate, written once.**
   - `telos_io::test_main(tests)` implements the runner of 12.7:
     - list, filter (substring) and `--exact`;
     - each test runs in a child process of the same executable, started with `--run-one <full name>`;
     - parallel up to `--jobs`, defaulting to the CPU count;
     - a per-test timeout, `--timeout` secs, default 60;
     - the child's stdout and stderr are captured and printed only for failures;
     - the cargo-style lines and summary;
     - exit codes as in 12.7.
   - `telos_io::assert_failed(...) -> !` prints the 12.5 format and exits 101 after restoring the terminal mode.
   - LLVM adapters in `telos-rt`: `telos_rt_test_main(table, count)` and `telos_rt_assert_failed`.
   - Do not add dependencies.
2. **Rust target.**
   - Lower `assert`, including `left:`/`right:` for `==` and `!=`, with each side evaluated once and deep structural equality.
   - Lower `?` in test bodies.
   - Emit a test-runner executable (`[[bin]] telos-test`) when tests are requested.
   - Normal builds stay byte-identical to the current goldens.
3. **LLVM target.**
   - Lower `assert` with deep-equality glue per type. `==` glue may already exist; reuse it.
   - Lower `?` in tests.
   - Emit a test main over a table of fn pointers.
   - Normal builds stay byte-identical to the current goldens.
4. **Driver.** `crates/telos/src/main.rs` gets `Commands::Test`:
   - usage: `telos test FILE… [--target rust|llvm] [--std DIR] [--out DIR] [FILTER] [--exact] [--list] [--jobs N] [--timeout SECS]`;
   - it runs the tests declared in the named files only. A std module can be the FILE, for example `telos test std/text.telos`. Make the driver handle an entry file that is itself a std module;
   - exit code 2 on a compile error.
5. **Tests.** `crates/telos/tests/telos_test.rs`, both targets, against fixture modules. It covers:
   - pass, fail and summary counts, and exit codes 0, 1 and 2;
   - the filter, `--exact` and `--list`;
   - `left`/`right` output for `==` on Int, Text and a record;
   - the custom message;
   - a failing `?`;
   - a non-exhaustive match panic;
   - `Process.exit(3)` inside a test, reported as "exited with code 3";
   - a timeout set with `--timeout 1`;
   - captured stdout shown only on failure;
   - a test that binds a `TcpListener` on port 0 and talks to it through a spawned task;
   - identical output lines on both targets, apart from timings.

   Coordinate fixture files with U1, who may already have some.
6. **Taskfile, docs and AGENTS.md.**
   - `task test-telos` runs `telos test` over `std/**/*.telos` and `examples/**/*.telos` on both targets, with `FILTER=…` passed through. `task test` and `task gate` include it. No comments in `Taskfile.yml`.
   - Add a command row to `AGENTS.md`, plus the convention line: "Telos behaviour is tested inline with `test` blocks; Rust tests cover the compiler and end-to-end binaries."
   - Update `docs/llvm.md` for the new lowering and ABI.

Acceptance:
- `task gate` is green, including `test-telos`. It has no Telos tests yet; a later agent writes the inline tests. Add one small inline test to `std/text.telos` only to prove the wiring, and tell the coordinator.
- The terminal goldens are byte-identical. The `terminald` and `tn` sizes are unchanged.
- Terminal and todo suites pass on both targets.

Rules:
- Use `CARGO_TARGET_DIR=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/target-j` for direct cargo calls.
- Prefix test runs with `ulimit -c 0;`.
- Release builds set `CARGO_PROFILE_RELEASE_DEBUG=0`.
- The rest of the rules are in `/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/rules.md`; read and follow them.
- Never call the GitLab API, with `curl` or anything else, using `$GITLAB_PERSONAL_TOKEN` or any other personal token of Timo's.
- A `terminald` on port 2323 belongs to Timo; leave it alone.

Final report, at most 12 lines:
- what landed;
- test counts;
- sizes and goldens unchanged (with numbers);
- deviations from section 12.
