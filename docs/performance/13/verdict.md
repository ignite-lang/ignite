# Gate-time budget (invariant 9): verdict B, not reachable without changing the gate

Budget: every required unit ≤1.10 × frozen 11b (`scripts/performance.py:147`, `:454`), warm
`cargo test --workspace` ≤10.8 s (`scripts/performance.py:464`, `docs/performance-plan.md:24,32`).
Sources are main at 8a71252 and branch `feat/slice13-local-apply` (bb783b3). No builds were run.
Abbreviations: REL = `docs/performance/12c/release-815cf74/`, REC = `REL/records.jsonl`.

## Measured
- M1. At release (815cf74), two units over 1.10: warm clippy 1.1135, no-run after touch 1.2814 (REL `summary.md:8,13`).
  Warm workspace is 1.0208 (`summary.md:9`); check 1.0647, plan 1.0589, inline 0.8611, ignite_test 0.5008, rustgen 0.9804 (`summary.md:5,6,10,14,15`).
- M2. Absolute 10.8 s: the unchanged 11b tree took 12.87, 11.22, 16.13, 13.51, 15.88 s in the
  same passes, at start loads 6.3 to 27.9 (REC lines 123, 150, 177, 204, 231). The candidate's best was 10.60 s at load 15.2 (REC line 213).
  The 10.8 s figure itself came from 11b at load 6 to 9 (`AGENTS.md:213`), 10.142 s at load 5.1 to 7.0 (`docs/performance/12b/report.md:13,26`).
- M3. Clippy and touch excess is in the driver crate `crates/ignite`: driver library frontend 0.22 to 0.60 s,
  E2E check 0.76 to 0.86 s, E2E no-run link 1.80 to 2.38 s (`12c/attribution/tooling-d8/summary.json`, `findings[0..1]`).
  A scope-map experiment on the checker gave no gain and was rejected (`12c/attribution/rejected-checker-scopes/README.md:8-11`).
- M4. Driver crate size (`git ls-tree` + `wc -l` per ref): src 2,193 (11b) / 7,462 (main) / 14,051 (slice 13).
  Its tests: 6,225 / 8,837 / 12,647 lines.
- M5. Slice 13 suites, from the unit reports in the caller's brief (not in the repo), under load 8 to 33:
  `apply_transition::` 343.66 s (8 tests), `apply::` 118 to 280 s, `apply_recovery::` 96.30 s, `apply_admission::` about 30 s, `apply_host::` 9 to 12 s.
  All five files are modules of the one E2E binary (branch `crates/ignite/tests/main.rs:6-9`).
- M6. `apply_transition` serialises all 8 tests on one file-wide mutex, taken at the start of each test (branch `apply_transition.rs:20`, `:503`, `:713`, …).
  Same pattern in `apply.rs:16` (`APPLY_BUILDS`). Build timeouts are 180 s and 120 s (`apply_transition.rs:19`, `apply.rs:15`).

## What decides it
- D1. The E2E binary runs its files in parallel, so its wall time is at least its longest file: 343.66 s (M5).
  That is 32 × the 10.8 s budget. The shortest slice-13 file, `apply_host` at 9 to 12 s, already uses the whole budget by itself.
- D2. If the mutex were narrowed to builds only (inferred, not measured), the wall would be about one test. 343.66 / 8 = 43 s per test on average, still 4 × 10.8 s.
  No change that keeps these tests in `cargo test --workspace` reaches 10.8 s. The plan forbids removing tests or "a smaller gate" (`docs/performance-plan.md:12-14`).
- D3. Even without slice 13, the absolute 10.8 s measures the host more than the code: 11b itself misses it at load above 6 (M2).

## Cost per feature
| feature | cost (measured unless marked) | source |
|---|---|---|
| slice 12 `std.control` | std parse/check/type +6.65/+7.74/+21.41 ms; check/plan back to 1.06 after 12c | `12b/report.md:54-55`, REL `summary.md:5-6` |
| slice 12 driver `run/` | clippy driver frontend +0.38 s, E2E no-run +0.58 s (d8 candidate); inferred: this is the M1 excess | `tooling-d8/summary.json` findings |
| slice 12 E2E `run`, `drain` | 4.304 s, 2.113 s per file; under the E2E wall | `12b/summary-final.md:70-71` |
| std growth (inline 102 to 148) | 0.663 to 1.140 s at 12b; 0.8611 × 11b after 12c | `12b/report.md:29`, REL `summary.md:10` |
| rustgen helper | 1.60 to 3.13 s at 12b; 0.9804 after 12c | `12b/report.md:60-61`, REL `summary.md:15` |
| E2E suites (pre-13) | 4.93 to 4.91 s execution, unchanged | `12b/report.md:60` |
| slice 13 `apply` | E2E wall ≥343.66 s (M5, D1). Driver src +6,589 lines and tests +3,810 lines (M4); inferred: clippy and touch ratios rise further | M4, M5 |

## Proposed budget (needs Timo's decision, because it changes the gate)
| unit | proposed | reason |
|---|---|---|
| check, plan, fmt, inline, todod ×4, no-run warm, rustgen ≤1.0, `ignite_test.rs` ≤4.6 s | unchanged vs 11b | all pass today (M1) |
| warm clippy | ≤1.30 × 11b | 1.1135 at 7,462 driver lines; slice 13 doubles the crate (inferred) |
| no-run after touch | ≤1.40 × 11b | 1.2814 today; the E2E binary grows 43 % with slice 13 (M4, inferred) |
| warm workspace, fast tier (all except live `apply*` files) | ≤1.10 × 11b paired; 10.8 s reported, not gating | ratio passes at 1.02 (M1); the absolute fails for 11b itself (M2) |
| live tier (`apply*`, run by `task gate` as its own step) | ≤360 s wall now; ≤120 s after the mutex covers builds only | 343.66 s measured; 120 s is inferred (D2), measure after the fix |
| cold gate | ≤1.10 × 11b for the fast tier | never collected at release (REL `README.md:35-37`); no evidence to move it |

Re-measure M5 at low load with `scripts/performance.py` once the build slot is free. Every slice-13 number here is from unit reports at load 8 to 33.

## At the close (2026-11-03)

- Decision on the proposed budget: none received; slice 13 is recorded as an exception (`AGENTS.md`, "Slice 13").
- M6 fixed in U6: the apply suites share one build per target and hold the lock only around it. `apply_transition::` 8 tests 363.50 s to 10 tests 273.78 s; `apply::` filter 675.80 s (3 deadline failures) to 448.83 s (46 passed), one cargo build on the host.
- `task gate` 410 s, exit 0; end-to-end binary 388.31 s (`AGENTS.md`, "Slice 13").
