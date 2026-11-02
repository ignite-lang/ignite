You work in /home/timo/projects/telos, the Ignite compiler: a Rust compiler with two targets (Rust and LLVM), a compile-time evaluator, a standard library in Ignite (`std/`) and a substrate in Rust. Read first:
- `AGENTS.md`, including the "Gate time" section;
- `docs/friction.md`;
- `docs/slice.md` sections 6.1, 14.5, 14.14, 15 (15.1 for the measuring method, 15.6, 15.14) and 16 (your contract, reviewed and accepted by the steering session; amendments A1 to A15 applied);
- `docs/vision.md` section 4.1 (row 12); read only: you do not edit `docs/vision.md` or `PLAN.md`;
- `docs/design.md` sections 4.1 to 4.3, 7.1, 7.4, 10.1, 11.7 and 11.8;
- `docs/actors.md` R8;
- `std/contract.ignite`, `std/policy.ignite`, `std/probe.ignite`, `std/deploy.ignite`, `std/protocol/telnet.ignite`, `std/net.ignite`, `examples/terminal/terminal.design.ignite`;
- the rules file `/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/s11_rules.md`; follow it.

You are P4: std, examples, records and acceptance of slice 12. You start when the coordinator tells you P1's skeleton has landed.

| agent | owns |
|---|---|
| P1 | `ignite-syntax`, `ignite-check`, `ignite-types`, `ignite-eval/src/{resolve,validate}.rs`, `docs/slice.md`; wrote the `std/control.ignite` type declarations in the skeleton |
| P2 | `crates/ignite-io/**`, `crates/ignite-rt/**`, the 16.13 lowering, `crates/ignite/src/run/{process,probe,clock,journal,control}.rs` |
| P3 | `crates/ignite-eval/src/{model,value,eval,explain}.rs`, `crates/ignite/src/run/{mod,engine,replay}.rs`, `crates/ignite/src/lib.rs`, `main.rs`, `crates/ignite/tests/**` |
| P4 (you) | `std/**` (after P1's skeleton), `examples/**`, `AGENTS.md`, `docs/friction.md`, the notes in `docs/design.md` named below |

`crates/ignite/tests/` is P3's: you write the trace fixture file there and ask P3 for the test that runs it. Changes to the types in `std/control.ignite` go by message between you and P3. The coordinator gives you the ids.

## Order (A12)

Units 1 to 3 first, starting with `std/control` and `std/probe`, because P3's engine steps them. Unit 4, the rollout reducer, is your last std unit: start it only after the coordinator tells you acceptance items 1 to 5 and 7 to 10 of 16.0 pass. If the slice runs long, the close reports the rollout as the carried item. Acceptance (unit 6) comes last.

## Units

1. Reducers in `std/control/*.ignite` (16.6 to 16.10): supervisor (16.7, with `Restart.readiness_timeout = 10 seconds`, O37), probe scheduler (16.8, slots from the target's first `Ready`, O40), contract evaluator (16.10, buckets of O33, `Standing`), consequences (16.11). Inline tests over synthetic event lists with chosen `at` values; no clock. Include A7's test: an `Evaluate` machine with a 30-day `Rolling` window and `interval = 5 seconds` steps 10,000 emissions in the evaluator under the default fuel per step; record its time. If the fuel does not hold, O33 becomes `window / 1,000`: tell P1 to record that in 16.19, and the test is the evidence.
2. Domain kinds and hooks: `K.machine` and `K.validate` on `TelnetServer`, `TelnetPrompt`, `Availability`, `ThresholdCompliance`, `SLA`; `minimum_completeness: Ratio = 90%` on the three contract kinds (O32); `meets(_, 0, 0)` becomes `false`, and the flipped inline test cites design 4.1: "Zero traffic does not automatically prove compliance" (A15); `fn standing(...)`; `fn on_deadline(text) -> Option<OnDeadline>` (O35); `validate` reasons for E120 (16.17). `ignite check` of the design file must stay exit 0 with no output (A14).
3. Probe and time: `TelnetPrompt` gains `executable` and `main` (16.8, O42); `Telnet.await_prompt` in `std/protocol/telnet.ignite`; the `Endpoint.of` declaration in `std/net.ignite`; `std/time.ignite` with UTC and Europe/Berlin (EU DST rule) month boundaries and inline tests across DST months (O36). The reducer trace fixture of 16.0 item 10.
4. Rollout reducer (16.12, last, see Order): the sequence of 16.12 with `AwaitRelease` on the `released` line (A5) and the declared disruption.
5. Records:
   - `AGENTS.md`, A1: extend the `crates/ignite-eval/` row with `run` and `replay` (it steps the reducers inside `ignite run` and `ignite replay`); "linked by no backend and no compiled program" stays true and stays in the row. Add layout rows for `std/control/`, `std/time.ignite` and `crates/ignite/src/run/`, and the `ignite run` and `ignite replay` rows in the commands table.
   - `AGENTS.md`, A11: the `examples/terminal/terminal.design.ignite` cell "`ignite build` does not build it" becomes "`ignite run` builds it; `task compile` builds `terminal.ignite`".
   - `AGENTS.md` slice-11 test counts (lines 174 to 177): the 11b record says the cargo count was 580 and slice 11's 784 counted the 204 inline tests in. If any line there still gives 784 as a cargo count, correct it so the file agrees with itself; as read on 2026-11-02 04:20 line 175 already says "784 passed (580 cargo + 2 × 102 inline)": then report "no change needed".
   - `AGENTS.md`: the slice-12 row in "Gate time" (unit 6).
   - `docs/design.md`: a note in 10.1 that `ignite run` and `ignite replay` exist as `docs/slice.md` 16.3 defines them; a note in 11.8 that slice 12 has four machines and folds the listener lifecycle into the supervisor (O44), and that the rollout's disruption is declared by the plan (O34).
   - `docs/friction.md`: mark the rows slice 12 solves; add the friction you hit.
6. Acceptance: 16.0 items 4 and 11 to 13, each verified with the command you ran, pass or fail.

## Measuring (16.1, item 12)

- The before tree is `/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/s12-before`: an rsync copy of `git ls-files -co --exclude-standard` of the working tree at slice-12 start (741 files, 5.8 MB), made by the coordinator; it is not a `git archive`, because the slice-11 and 11b state is uncommitted. Do not re-create it and do not modify it; give it its own target dir (`target-s12-before` in the scratch dir).
- The interleaved pairs of 15.1: for each timed unit (warm gate, the warm `cargo test --workspace` step, each file of `crates/ignite/tests/`, `todod` per compile step on both targets dev and release, `ignite check` and `ignite plan` of the design file), one run on the before tree, then one on the slice-12 tree, alternating, at least 3 pairs; record the 1-minute load for every run, drop a pair whose loads differ by more than 5, report the median of the pair ratios next to the absolute times. Write the method into the row's notes.
- The warm test step stays under slice 11b's 10.8 s (median, load 6 to 9).
- Absolute times without a before: `ignite run` to the first `ready` (warm build), `ignite replay` of a 10,000-record journal, the A7 test.
- A rise over 10 % is a deviation to explain; O26 applies.

## Gate

- `cargo fmt --check` is clean; clippy `-D warnings` is clean on the workspace.
- `cargo test --workspace` is green.
- `task test-ignite` is green on both targets.
- `task gate` is green.
- `task compile`, `task compile-llvm`, `task compile-todo` and `task compile-todo-llvm` succeed.

## Rules

- `CARGO_TARGET_DIR=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/target-s12-p4`.
- Domain nouns stay in std (invariant 6): no Rust change of yours.
- Everything else is in the rules file.

## Final report to the coordinator, at most 15 lines

- std lines added (code and tests separately), inline test count per target;
- the A7 fuel result and time;
- `AGENTS.md` and design notes written;
- 16.0 items 4 and 11 to 13, pass or fail, each with its command;
- the slice-12 gate-time row;
- friction rows solved and opened;
- deviations from section 16.
