You work in /home/timo/projects/telos, the Ignite compiler: Rust, two targets (Rust via `crates/ignite-rustgen`, LLVM via `crates/ignite-llvm` and `crates/ignite-rt`), a compile-time evaluator (`crates/ignite-eval`), a std library in Ignite (`std/`). Read first: `AGENTS.md`; `docs/slice.md` sections 13.11 (row d), 14.3 (Map), 14.14 and 15.14; `docs/friction.md` (the four rows P4 added for these bugs); `docs/llvm.md`; the rules file `/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/s11_rules.md` (follow it).

You are the slice-11b agent. Slice 11 is closed and its gate is green. You are alone in the tree. The steering session decided (2026-11-02 03:03) that these four target bugs are fixed before any slice 12 work, because both targets must agree (vision wave A0).

## The four bugs

| # | bug | fix |
|---|---|---|
| a | The Rust target loses `lists[0].push(9)` on `var lists = [[1], [2]]`: the generated code clones the indexed receiver, so the program prints `[[1], [2]]`; LLVM and the evaluator write through (`[[1, 9], [2]]`), as 13.11 row d requires. | rustgen writes through an indexed receiver (and a field-of-index place, and nested indexes), matching LLVM and the evaluator. Update the 13.11 row d text if it does not already say this for indexed receivers. |
| b | `Map<Color, Int>` with a user enum key fails in rustgen: no `Ord` derive (rustc E0277). LLVM is fine; 14.3 allows any key with structural order. | rustgen derives what a map key needs (`Ord`, `PartialOrd`, `Eq`) for user enums and records used as map keys, with the same order as LLVM and the evaluator (variant declaration order, then payload; fields in declaration order). |
| c | `Exposure` as a fn value type: rustgen emits no `Exposure` type (rustc E0425); LLVM panics at `crates/ignite-llvm/src/layout.rs:233`. | Both targets lower `Exposure` (and check the other substrate builtins `TcpListener`, `TcpStream` the same way) as a fn parameter, return and local type. A type that has no value at run time is a checker diagnostic, never a backend panic. |
| d | A user record named `Box` shadows std `Box` in generated Rust (E0072). | Generated Rust does not collide with Rust std or prelude names: use the scheme the generator already uses for other builtins (a prefix or a `::std::…` path), whichever it is. Cover `Box`, `Vec`, `Option`, `Result`, `String` and `Ok`/`Err`/`Some`/`None` as user names where the language allows them. |

P2 found these while writing the equivalence fixture `crates/ignite/tests/fixtures/eval_equivalence.ignite` and dropped three fns from it because of them. Put those fns back (an indexed push, a map with a user-enum key, a record named `Box`) so the evaluator-against-both-targets test covers them.

## Two carried items from slice 11's close

- Warm test step target, steering decision: under 8 s at load under 5. Slice 11 reached 13.6 s, measured in pairs against 23.0 s on 60aef38. `crates/ignite/tests/ignite_test.rs` sets the wall time of the e2e binary at 8.6 s, including `timeout_kills_a_test` at 2.6 s, which waits on a 1 s timeout per target, and `overflow…` at 1.7 s. Cut it to the target: shorter timeouts in tests, one `ignite test` invocation where several run the same build, parallel instead of serial. Keep one test per behaviour, and name each changed test with its time before and after. Measure the warm `cargo test --workspace` step as P4 did (interleaved pairs against the `s11-p4-before` tree in the scratch dir, or at load under 5).
- `docs/design.md` §11.2 still says the design file "does not compile yet"; since slice 10 `ignite check` and since slice 11 `ignite plan` accept it. Correct that sentence.

## Tests

- One regression test per bug, run on both targets where the bug is target-independent (a build-and-run test or an inline `test` block run by `ignite test` on both targets), plus the restored equivalence fns.
- Rust and LLVM goldens: a golden that changes is re-blessed with `IGNITE_BLESS=1`; explain every golden diff in your report. Programs without the affected constructs stay byte-identical.

## Records

- Close the four rows in `docs/friction.md` (mark them solved by slice 11b, with the test names).
- Add one line to `docs/slice.md` 15.14: "Slice 11b (2026-11-02): fixed the four target bugs of friction rows N to M; tests: …".

## Gate

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --no-fail-fast`, `task test-ignite` (both targets), `task gate`, and the four tasks `compile`, `compile-llvm`, `compile-todo`, `compile-todo-llvm`: all exit 0.
- Invariant 9: `todod` build time on both targets, dev and release, measured interleaved against a `git archive 60aef38` copy, the method of `docs/slice.md` 15.1; a rise over 10 % against slice 11's numbers in `AGENTS.md` is a deviation to explain.

## Rules

- `CARGO_TARGET_DIR=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/target-11b` for direct cargo calls.
- Every cargo call sets `CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_INCREMENTAL=0`; release builds also `CARGO_PROFILE_RELEASE_DEBUG=0`. Prefix test runs with `ulimit -c 0;`. `df -h /` before your first build; stop under 20G.
- One cargo or task run at a time; anything over about 30 s in the background, waited on by PID or a final `EXIT=` line you write.
- No git command that changes state; no commits.
- Never call the GitLab API, with `curl` or anything else, using `$GITLAB_PERSONAL_TOKEN` or any other personal token of Timo's.
- No work files in /tmp or /dev/shm; no heredocs or `python3 -c`; never delete a `target/` directory; no comments in config files; leave the `terminald` on port 2323 alone; kill your own processes by explicit PID before you finish.
- On an HTTP 429 or usage-limit error, continue; do not reduce scope.

## Final report to the coordinator, at most 12 lines

- each bug: the fix (file and function) and its regression test names;
- golden diffs with the reason;
- test counts (cargo, inline per target);
- the gate commands and exit codes;
- todod times against slice 11;
- deviations.
