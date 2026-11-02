# Steering review of the slice 12 draft (2026-11-02 04:10)

Draft: `/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/slice12_contract_draft.md`
(555 lines, read in full). Checked against `docs/vision.md` 4.1 row 12, section 6, T1 to T6,
invariants 5, 6, 9; `docs/design.md` 4.1, 7.1, 10.1, 11.7, 11.8; `AGENTS.md` layout rules.

Verdict: accepted with the amendments below. The defaults O27 to O43 stand except where a row
below changes them. Copy the amended text into `docs/slice.md` section 16, then dispatch.

| # | section | amendment |
|---|---|---|
| A1 | O27 | Accepted. Steering amends `docs/vision.md` 4.1 ("runs on threads and the Rust target" becomes: reducers stepped by the evaluator inside `ignite run`, compiled on both targets in the trace fixture of item 10). P4 extends the `crates/ignite-eval/` row of `AGENTS.md` with `run` and `replay`; "linked by no backend and no compiled program" stays true and stays in the row. |
| A2 | O29, O41, 16.4 | Tests use `--settle 1 s`, not 200 ms. O41 becomes: all `ignite run` end-to-end tests together add at most 6 s wall to `crates/ignite/tests`, and only the hung-probe scenario (one test, one target) pays a settle wait. Tests build the design file once per target into one shared `--state` and never rebuild. Tests assert the `at` of `Ready`, `AttemptDone` and `Started` only as "the instant of the dispatch". Reason: load on this host was 24 to 32 during 11b's gate; a 200 ms cap on spawn + connect + prompt moves a healthy attempt to a later instant and breaks items 1, 3 and 8. |
| A3 | 16.2 | `PR_SET_PDEATHSIG` is tied to the thread that forked (prctl(2): the signal is sent when that thread terminates, not when the process does). The process adapter spawns every child from one thread that lives for the whole run, and the contract says so. |
| A4 | 16.8 | Gap: slots start at the target's first `Ready`, but each event is stepped by one addressed reducer. Rule to add: the driver routes a member's lifecycle events (`Started`, `Ready`, `Exited`, `Signalled`, `SpawnFailed`) to its supervisor and to every scheduler with an `edges.readiness` edge to that member. No readiness signal in `observations`. |
| A5 | 16.12 step 3, 16.13 | `Released` comes from a stdout line `released <address>:<port>` that the drain prints after the listener is closed (the `listening on` rule in reverse), not from connect-polling. Reason: polling connects open sessions on the draining process and the measured disruption interval depends on the poll period. The `AwaitRelease` effect stays; its adapter reads the line. |
| A6 | 16.13 drain | Existing tests end servers with `Child::kill` (SIGKILL; verified by grep over `crates/*/tests`), so the SIGTERM change touches no existing test. Keep the second-SIGTERM rule. State in 16.13 that outside `ignite run` the wait for live tasks is unbounded, so the behaviour is written down. |
| A7 | O33, E128 | Add one inline test: an `Evaluate` machine with a 30-day `Rolling` window and `interval = 5 seconds` steps 10,000 emissions in the evaluator under the default fuel per step; item 12 records its time. If fuel does not hold, O33 becomes `window / 1,000` and the test is the evidence. |
| A8 | 16.19 | New row O44: design 11.8 lists five machines; slice 12 has four and folds "Listener lifecycle" (observe, bind or adopt, report ready, release) into the supervisor (`listening on` gives `Ready`, the `released` line gives `Released`). Quote the design row so slice 13 (adopt and bind ownership) finds it. |
| A9 | O34 | Accepted. Add the design 11.8 sentence verbatim to the row: "Until it does, planning rejects that rollout binding or requires an explicit stop-and-start strategy with declared disruption." (verbatim from `docs/design.md` 11.8; the first version of this row paraphrased it) The plan's `machines[].disruption` is the declaration slice 12 chooses; a user field is not added. |
| A10 | 16.3 `WorkstationExecutable` | The design file's `client: Application` (std.cli) has no `main`, and `ignite build` of the design file emits `terminald` only (verified: LLVM build with the 11b driver, exit 0, `bin/terminald`). Rule: a `WorkstationExecutable` member whose kind has `main` is built and its path printed; one without `main` is listed as "declared, not buildable in slice 12" in the `status` output and the journal's first `segment`. Not E129. |
| A11 | 16.0 item 1 | Verified feasible: `ignite build examples/terminal/terminal.design.ignite --target llvm` exits 0 today. `AGENTS.md` says "`ignite build` does not build it"; P4 rewrites that cell to "`ignite run` builds it; `task compile` builds `terminal.ignite`". |
| A12 | 16.18 order | 16.12 (`DrainRollout`) and the drain row of 16.13 are the last units of P2 and P4, started after acceptance items 1 to 5 and 7 to 10 pass. Reason: the slice is larger than 11 (two commands, four machines, ten codes, substrate signals); if it runs long, the close reports rollout as the carried item and the rest closes. Not a cut: vision row 12 keeps `DrainRollout` in the slice. |
| A13 | `ignite replay` | Accepted as the determinism check of item 7; not in vision row 12 or design 10.1. Design line "Interpreter and native execution share semantic tests and replay traces" (section 7) covers it. P4's design 10.1 note lists `run` and `replay`; steering adds `replay` to vision row 12 and `PLAN.md`. |
| A14 | 16.17 E121 to E124 | Phase "check" means `ignite check` of the design file must stay exit 0 with no output (`cli::check_design_file_is_silent`). P1 runs that test first after the validators land. |
| A15 | O32 | Accepted; design 4.1 "Zero traffic does not automatically prove compliance" is the source. The flipped inline test cites it. |

Records steering owns and changes now: `docs/vision.md` (4.1 paragraph, row 11 count, row 12
`replay`), `PLAN.md` (rows 11, 11b, 12, "Now" table). Records telos owns: `AGENTS.md` lines 174
and 177 say "784 passed" for slice 11; the 11b record already states the cargo count was 580
and the 784 counted the 204 inline tests. Correct the two lines so the file does not disagree
with itself.

Model: Fable (`claude-fable-5-1`) for every agent of the slice, as named at kickoff.
