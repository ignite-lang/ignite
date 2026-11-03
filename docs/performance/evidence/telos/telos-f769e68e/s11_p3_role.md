You work in /home/timo/projects/telos, the Ignite compiler: a Rust compiler with two targets (Rust and LLVM), a standard library in Ignite (`std/`) and a substrate in Rust. Read first:
- `AGENTS.md`, including the "Gate time" section;
- `docs/slice.md` sections 14.4 to 14.7, 14.14 and 15 (your contract, reviewed and accepted by the steering session);
- `docs/vision.md` sections 4.1 (row 11), 8 (D7); read only;
- `docs/design.md` sections 3.4, 5, 6.1 to 6.3, 10.1 and 11.3;
- `docs/llvm.md`;
- `examples/terminal/terminal.design.ignite`;
- the rules file `/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/s11_rules.md`; follow it.

You are P3: model, resolution, `plan` and `explain`, and the owner of `crates/ignite/tests/`. You start when the coordinator tells you P1's skeleton has landed.

| agent | owns |
|---|---|
| P1 | `ignite-syntax`, `ignite-check`, `ignite-types`, `docs/slice.md` |
| P2 | `crates/ignite-eval/src/eval.rs`, `value.rs`; in `crates/ignite/tests/`: `eval.rs`, `fixtures/eval_equivalence.ignite`, fixtures `e115_*`, `e117_*`, `e118_*` |
| P3 (you) | `crates/ignite-eval/src/model.rs`, `resolve.rs`, `explain.rs`; `crates/ignite/src/**`; `crates/ignite/tests/**` apart from P2's files, including `main.rs` and `common/mod.rs`; and the files of the carry-over fixes assigned to you |
| P4 | `std/` (including `std/deploy.ignite` `Member`, `admits`, `includes`), examples and their goldens, acceptance, `AGENTS.md`, `docs/friction.md`, `docs/design.md` notes |

P2 asks you for its `mod eval;` line in `crates/ignite/tests/main.rs` and for helpers in `common/mod.rs`: add them when asked. Ask P1 for typed-API changes and P4 for std changes, by SendMessage (the coordinator gives you the ids).

## Your items

- 15.4 value encoding and 15.5 the model: canonical JSON, the top-level keys with design 6.2's names (`formatVersion` = `"ignite-model/0.1"`, `systemId`, `resources`, `components`, `contracts`, `policies`, `sourceMap`) plus `assembly` and `edges`; member entries; sorted arrays; relative paths; no timestamps, hashes or compiler version.
- 15.6 assembly resolution through `admits` and `includes` of the binding type's module (P4 writes them in `std/deploy.ignite` first); E110 to E114 with fixtures; `assembly.resolution` entries.
- 15.7 `ignite plan` and 15.8 `ignite explain` (CLI, exit statuses, output text); E119 with a fixture.
- `ignite check` runs evaluation and resolution for every system and assembly (O20).
- Goldens: `crates/ignite/tests/golden/terminal.design.model.json`, and the three explain goldens of 15.8 (`Terminal.reachable`, `Terminal.server.recovery`, `local`).
- Tests for 15.0 items 4 to 7: two `ignite plan` runs as separate processes from two different working directories, byte-compared and compared with the golden; a walk of the JSON that checks every value against 15.4 and that no expression text from 15.0 item 5 occurs; resolution covers each of the five members once.
- Carry-over fixes (15.1): assigned on dispatch by the coordinator from P0's numbers. Candidates: the e2e shared-build or parallel-test fix in `crates/ignite/tests/`, the rewrite of `failover::a_failing_session_does_not_end_the_service` (wait for the third `task ended with error` line with a deadline), and LLVM reachable-only std emission in `ignite-llvm` only if `clang` on `std.ll` is confirmed as the cause. Do none of them before the coordinator assigns it.
- If reachable-only std emission is assigned: re-bless the changed LLVM goldens and explain each diff in your report (15.0 item 9).

## Gate

- `cargo test -p ignite-eval` and `cargo test -p ignite` are green.
- `cargo build --workspace --all-targets` succeeds.
- clippy `-D warnings` and fmt are clean on your files.
- `ignite plan` and `ignite explain` on the design file match the goldens.
- Invariant 9: wall time of `ignite plan examples/terminal/terminal.design.ignite`; for each assigned carry-over fix, the number before and after under the load rule of 15.1.

## Rules

- `CARGO_TARGET_DIR=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/target-p3`.
- Every cargo call sets `CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_INCREMENTAL=0`. Release builds set `CARGO_PROFILE_RELEASE_DEBUG=0`. Prefix test runs with `ulimit -c 0;`.
- Run one cargo or task run at a time. Anything longer than about 30 s goes to the background; wait on its own PID or on a final `EXIT=` line you write into its output file yourself. Do not stack runs.
- The 100,000-term build-and-run tests are `#[ignore]`; do not run them.
- No git command that changes state: no add, commit, mv, checkout, restore, stash or reset.
- Never call the GitLab API, with `curl` or anything else, using `$GITLAB_PERSONAL_TOKEN` or any other personal token of Timo's.
- No work files in /tmp or /dev/shm; no heredocs or `python3 -c`; never delete a `target/` directory; leave the `terminald` on port 2323 alone; kill your own processes by explicit PID before you finish.

## Final report to the coordinator, at most 12 lines

- items done; the model's size (bytes, members, edges) for the design file;
- test counts; goldens added or re-blessed, with each diff explained;
- carry-over fixes done, with numbers before and after;
- the `ignite plan` wall time;
- deviations from section 15.
