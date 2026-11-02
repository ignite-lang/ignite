You work in /home/timo/projects/telos, the Ignite compiler: a Rust compiler with two targets (Rust and LLVM), a compile-time evaluator (`crates/ignite-eval`), a standard library in Ignite (`std/`) and a substrate in Rust. Read first:
- `AGENTS.md`, including the "Gate time" section and the one-test-binary-per-crate convention;
- `docs/slice.md` sections 6.1, 14.8, 15 (especially 15.3 to 15.8 and 15.14) and 16 (your contract, reviewed and accepted by the steering session; amendments A1 to A15 applied);
- `docs/design.md` sections 2, 3.4, 4.1, 6.2, 7, 7.1, 10.1, 11.7 and 11.8;
- `docs/actors.md` R8;
- `examples/terminal/terminal.design.ignite`;
- the rules file `/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/s11_rules.md`; follow it.

You are P3: driver loop, model, replay, and the owner of `crates/ignite/tests/`. You start when the coordinator tells you P1's skeleton has landed.

| agent | owns |
|---|---|
| P1 | `ignite-syntax`, `ignite-check`, `ignite-types`, `ignite-eval/src/{resolve,validate}.rs`, `docs/slice.md` |
| P2 | `crates/ignite-io/**`, `crates/ignite-rt/**`, the 16.13 lowering, `crates/ignite/src/run/{process,probe,clock,journal,control}.rs` |
| P3 (you) | `crates/ignite-eval/src/{model,value,eval,explain}.rs`; `crates/ignite/src/run/{mod,engine,replay}.rs`, `crates/ignite/src/lib.rs`, `main.rs`; `crates/ignite/tests/**`, including `main.rs` and `common/mod.rs` |
| P4 | `std/**` (including the `std/control` reducers your engine steps), `examples/**`, `AGENTS.md`, `docs/friction.md`, design notes, acceptance |

Add the `mod` lines, helpers and fixture test lines P1, P2 and P4 ask for. Agree the adapter interface with P2 first (P2 keeps it in `run/control.rs`). Changes to `std/control.ignite` go by message between you and P4. The coordinator gives you the ids.

## Your items

- `ignite-eval`: `Value::decode` (inverse of 15.4), `Evaluator::call`, and `Model.machines` and `Model.observations` with the edge classes `provisioning`, `readiness`, `recovery` (16.14, additive, `ignite-model/0.1`); `ignite explain` of machine ids.
- The engine (`run/engine.rs`, 16.2, 16.6, 16.9): one ordered queue; journal each event, step it through its reducers, journal each effect, then dispatch. Routing: signals by `subscribes` in machine-id byte order; A4: a member's lifecycle events (`Started`, `Ready`, `Exited`, `Signalled`, `SpawnFailed`) go to its supervisor and to every scheduler with an `edges.readiness` edge to that member; readiness is not a signal and not in `observations`. Ref resolution through `provisioning` edges for the `--ref` arguments. A reducer failure is E128: that machine stops, the run continues and exits 1.
- `run` and `replay` in `lib.rs` (16.3, 16.15): plan, check E120 to E125 and E129, build into `<build>` (default `<state>/build/`, a no-op when sources are unchanged) and copy into `<state>/bin/<generation>/`, open the journal, start the machines. A10: a `WorkstationExecutable` member whose kind has `main` is built and its path printed; one without `main` is listed as "declared, not buildable in slice 12" in `status` and in the first `segment` record; not E129. `ignite replay`: E126 on a header mismatch, E127 with the first diverging `seq`. Design 7 ("Interpreter and native execution share semantic tests and replay traces") is its source (A13).
- E125 to E128 with fixtures `e125_nothing_to_run`, `e126_journal_mismatch`, `e127_replay_diverged`, `e128_reducer_failed`.
- The model golden of the design file re-blessed with the diff explained; the terminal and todo model goldens change only by empty `machines` and `observations`.
- End-to-end tests of 16.0 items 1 to 3 and 5 to 9, with these limits (A2, O41):
  - all `ignite run` e2e tests together add at most 6 s wall to `crates/ignite/tests`;
  - only the hung-probe test (item 3 with item 5's month boundary, one test, one target) pays a settle wait; tests pass `--settle 1 s`;
  - the design file is built once per target into one shared `--build` and never rebuilt; each test has its own `--state`;
  - the `at` of `Ready`, `AttemptDone` and `Started` is asserted only as "the instant of the dispatch";
  - no test uses port 2323 (`--port Terminal.ingress=0`);
  - every journal a test writes is replayed (item 7).
- Item 6 (rollout) is the last unit, after P2's and P4's drain units (A12).

## Gate

- `cargo test -p ignite-eval` and `cargo test -p ignite` are green.
- `cargo build --workspace --all-targets` succeeds.
- clippy `-D warnings` and fmt are clean on your files.
- `ignite plan` and `ignite explain` of the design file match the goldens; `cli::check_design_file_is_silent` stays green.
- Invariant 9: `ignite plan` of the design file before (on `/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/s12-before`, do not modify it) and after, interleaved as in 15.1; the absolute times of `ignite run` to the first `ready` (warm build) and of `ignite replay` of a 10,000-record journal; the wall time of each new test file.

## Rules

- `CARGO_TARGET_DIR=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/target-s12-p3`.
- The driver knows only the `std.control` types and the hook names `machine` and `validate`; no domain noun in Rust (invariant 6).
- Everything else is in the rules file.

## Final report to the coordinator, at most 12 lines

- items done; the model's size for the design file (bytes, machines, edges);
- goldens re-blessed, each diff explained;
- test counts; wall time of the new e2e tests against the 6 s budget;
- the plan, run and replay times;
- deviations from section 16.
