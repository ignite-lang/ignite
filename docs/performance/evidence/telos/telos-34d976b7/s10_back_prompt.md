You work in /home/timo/projects/telos, a Rust compiler for the Telos language with two targets. Read these first:
- `AGENTS.md`, including the "Gate time" section;
- `docs/llvm.md`;
- `docs/slice.md` sections 6.1, 7, 12.5, 13 and 14. Section 14 is the slice-10 contract, reviewed by the steering session. P1, the frontend agent, owns it; its id is given below;
- `docs/vision.md` section 7 (T1, T6) and 10.3 (R2 to R5).

The agents and their files:

| agent | owns |
|---|---|
| P1 | `telos-syntax`, `telos-check`, `telos-types`, `docs/slice.md` |
| P2 | `crates/telos-rustgen/**`, `crates/telos-io/**` |
| P3 | `crates/telos-llvm/**`, `crates/telos-rt/**`, `docs/llvm.md` |

Edit only your own files. Ask P1 by SendMessage for any API you need. Tell the other backend agent about any shared decision, so both targets print, compare and fail identically. That covers the failure report text of 14.8, Map debug text and iteration order, quantity debug text, and the order in which named arguments are evaluated.

Replace every `unimplemented!("slice 10: …")` placeholder in your crates (list in `docs/slice.md` 14.14). Your items are given below the line.

Each item gets a golden or a build-and-run test on your target. Builds of programs that use no slice-10 feature stay byte-identical to the current goldens; explain each diff that is not.

Headline acceptance (14.0.3): `terminald` survives a session whose handler fails an `assert`, overflows or indexes out of range. Your target must show it in an end-to-end test over a real socket. The test fixture may add a failing command to a copy of the terminal program; do not change `examples/`.

Gate:
- the tests of your crates are green;
- `cargo test -p telos` is green;
- `cargo build --workspace --all-targets` succeeds;
- clippy `-D warnings` and fmt are clean on your crates;
- invariant 9: the `todod` build time on your target, before and after.

If the gate fails only in another agent's files, report that and do not fix it. The tree is shared, so a run may fail on another agent's work in progress; wait and re-run.

Rules:
- Use `CARGO_TARGET_DIR=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/<your target dir>`.
- Every cargo call sets `CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_INCREMENTAL=0`. Release builds set `CARGO_PROFILE_RELEASE_DEBUG=0`.
- Prefix test runs with `ulimit -c 0;`.
- Run one cargo or task run at a time. Anything longer than about 30 s goes to the background; wait on its own PID or on a final `EXIT=` line you write into its output file yourself. Do not stack runs.
- The 100,000-term build-and-run tests are `#[ignore]`; do not run them.
- The rest of the rules are in `/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/rules.md`; read and follow them.
- No git command that changes state: no add, commit, checkout, restore, stash or reset.
- Never call the GitLab API, with `curl` or anything else, using `$GITLAB_PERSONAL_TOKEN` or any other personal token of Timo's.
- A `terminald` on port 2323 belongs to Timo; leave it alone. Use free ports in tests.
- No work files in /tmp; use the scratch directory.
- Kill your own processes by explicit PID before you finish.

Final report, at most 12 lines:
- items done;
- test counts;
- golden and size changes with numbers;
- the `todod` build time before and after;
- deviations from section 14.

---
