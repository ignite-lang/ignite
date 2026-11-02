You work in /home/timo/projects/telos, the Ignite compiler (Rust; two targets, Rust via `crates/ignite-rustgen` and LLVM via `crates/ignite-llvm` + `crates/ignite-rt`; evaluator `crates/ignite-eval`; std in Ignite under `std/`). Read first: `AGENTS.md` (layout, commands, the "Gate time" section incl. the slice-12 rows), `docs/slice.md` sections 15.1 (measurement method) and 16.22 (your contract, approved by the steering session), `docs/llvm.md` section 1 (reachable-only std on LLVM). Follow the rules file `/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/s11_rules.md` (it applies to slice 12b too).

Slice 12b has three agents. Ownership:

| agent | owns |
|---|---|
| R1 frontend | `crates/ignite/src/**`, and `ignite-check`, `ignite-types`, `ignite-eval` where the reached set needs support; the implementation-record lines of 16.22 in `docs/slice.md` |
| R2 Rust target | `crates/ignite-rustgen/**` incl. its goldens |
| R3 tests and measurement | `crates/ignite/tests/**`, `AGENTS.md`, the 15.1 pairs |

The scratch directory is `$S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6`. Your target dir is `$S/target-12b-<your role, r1|r2|r3>`. Message the other agents with SendMessage; the coordinator gives you their ids in a follow-up message if you need them, or ask the coordinator.

Shared facts:
- Measured causes of the rises (item 12): every compiler run loads and checks all of `std/` (3,033 to 5,135 lines); the generated `ignite-std` crate holds every std module (2,191 to 3,792 lines); `ignite_test.rs` 6.6 to 9.8 s.
- No diagnostic, golden or behaviour change, except rustgen goldens shrinking by dropped std items (each listed). A diagnostic a program no longer sees because it sits in an unreached std module is a finding to report, never silenced.
- Measurement trees: `$S/s12-before` with `$S/target-s12-before` (the 11b baseline), `$S/s12-p4-after2` with `$S/target-s12-p4-after2` (the slice-12 close). P4's scripts are in `$S/s12-p4-scripts/` (`remeasure.sh`, `pass.sh`, `pairs.py`, `finalgate.sh`); reuse them.

Report to the coordinator at the end, at most 10 lines: files changed, what was measured with which command, test counts, deviations, findings. No commits.
