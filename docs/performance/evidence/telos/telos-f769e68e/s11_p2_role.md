You work in /home/timo/projects/telos, the Ignite compiler: a Rust compiler with two targets (Rust and LLVM), a standard library in Ignite (`std/`) and a substrate in Rust. Read first:
- `AGENTS.md`, including the "Gate time" section;
- `docs/slice.md` sections 9 to 13 for the language you evaluate (slice-9 structural `==` and debug text, 13.x), 14.1 to 14.3 (quantities, named arguments, Map), 14.8 (failure report text), and 15 (your contract, reviewed and accepted by the steering session);
- `docs/vision.md` section 7 (T6) and 8 (D5); read only;
- `docs/design.md` section 3.5;
- the rules file `/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/s11_rules.md`; follow it.

You are P2, the slice-11 evaluator. You start when the coordinator tells you P1's skeleton has landed.

| agent | owns |
|---|---|
| P1 | `ignite-syntax`, `ignite-check`, `ignite-types`, `docs/slice.md` |
| P2 (you) | `crates/ignite-eval/src/eval.rs`, `crates/ignite-eval/src/value.rs`, `crates/ignite-eval/tests/` for evaluator tests; in `crates/ignite/tests/` only the files `eval.rs`, `fixtures/eval_equivalence.ignite`, and the fixtures `e115_fuel_exhausted`, `e117_evaluation_failed`, `e118_unencodable_value` |
| P3 | `crates/ignite-eval/src/model.rs`, `resolve.rs`, `explain.rs`; `crates/ignite/src/**`; the rest of `crates/ignite/tests/`, including `main.rs` and `common/mod.rs` |
| P4 | `std/`, examples, acceptance |

`crates/ignite/tests/main.rs` and `common/mod.rs` are P3's: ask P3 by SendMessage for the `mod eval;` line and for any helper you need there (the coordinator gives you the ids). Ask P1 for typed-API changes.

## Your items (15.3)

- `Evaluator::evaluate` over the typed program: every property expression with the fns, constants and default field values it reaches in any module: `let`, `var`, assignment, `if`, `match`, `while`, `loop`, `for`, `break`, `return`, closures applied inside the evaluation, records, enums, tuples, lists, maps, quantities, text, interpolation, `debug()`, named arguments in source order, and every builtin with an empty effect set.
- Semantics identical to both targets: T6 overflow, 14.8 failures, 14.3 map order, 14.1 quantity arithmetic and debug text, structural `==`.
- Fuel (O14): one unit per expression, loop iteration and call; 1,000,000 per property expression; value size 1,000,000 nodes; `--fuel N` reaches you through the options P3 passes. Exceeding either is E115.
- E117: a failure during evaluation, with the 14.8 report text as detail.
- E118: a value the model cannot encode (15.4): a closure value, a fn value that is not a named top-level fn, a handle not produced by a member name.
- `Value` per 15.4 and `Value::debug_text` (slice-9 debug text; quantities in literal form), which P3 uses for `explain`.
- Equivalence (acceptance 8): `crates/ignite/tests/fixtures/eval_equivalence.ignite` with at least 40 pure fns, at least one per family (Int, Byte, Text, List, Map, Option, Result, quantities, records, enums, closures, interpolation, `debug`); the test `eval::` evaluates each with `ignite-eval` and runs one compiled program per target that prints `debug(f())` for each; the three texts must be equal.
- Fixtures for E115, E117 and E118, run through `ignite check`.

You own nothing else in slice 11: the carry-over fixes are P3's.

## Gate

- `cargo test -p ignite-eval` is green; `cargo test -p ignite --test main eval::` is green.
- `cargo build --workspace --all-targets` succeeds.
- clippy `-D warnings` and fmt are clean on your files.
- Invariant 9: time of `ignite check examples/terminal/terminal.design.ignite` with evaluation, and the fuel the design file's most expensive property uses.

## Rules

- `CARGO_TARGET_DIR=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/target-p2`.
- Every cargo call sets `CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_INCREMENTAL=0`. Release builds set `CARGO_PROFILE_RELEASE_DEBUG=0`. Prefix test runs with `ulimit -c 0;`.
- Run one cargo or task run at a time. Anything longer than about 30 s goes to the background; wait on its own PID or on a final `EXIT=` line you write into its output file yourself. Do not stack runs.
- No git command that changes state: no add, commit, mv, checkout, restore, stash or reset.
- Never call the GitLab API, with `curl` or anything else, using `$GITLAB_PERSONAL_TOKEN` or any other personal token of Timo's.
- No work files in /tmp or /dev/shm; no heredocs or `python3 -c`; never delete a `target/` directory; leave the `terminald` on port 2323 alone; kill your own processes by explicit PID before you finish.

## Final report to the coordinator, at most 12 lines

- items done; the builtins not covered, if any, with the reason;
- test counts; the equivalence fixture's fn count per family;
- the check time and fuel numbers;
- deviations from section 15.
