# Slice 15 gate time (`docs/slice.md` 19.17)

Measured 2026-11-04 00:20 to 02:41 local time (UTC+1). 20 cores, shared with other sessions; start
loads 2.6 to 16.6. sccache was on. Every command ran through the session's build-slot wrapper
(`CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=line-tables-only`, `RUST_TEST_THREADS=4`,
`CARGO_BUILD_JOBS=8`, `nice -n 10`, `ionice -c3`). The record is in `AGENTS.md`, "Slice 15".

## Trees and harness

- **Before:** `296f849`, the slice-15 start.
- **After:** `c20f04b`, the claims with the adversary's fixes. The next commit, `621070e`, changes
  only `tools/ignite-perf`.
- Both trees are exported by `ignite-perf` and built in their own targets, with
  `--comparison ""` and no published comparison.
- `ignite-perf` at `621070e`, harness SHA256 `bb2deecd…6621` (`records/run.json`).
- `621070e` fixes the harness's export. Before it, every mode stopped at
  `Unsupported source archive member: docs/performance/evidence/.gitattributes`, because each
  tree since `9ad4640` carries that file.

## Method

- The method of 19.17: modes `cold`, `warm`, `programs` and `cli`, then `summarize`
  (`scripts/perf.sh`).
- One cold pair, with one retry. Then three warm pairs and up to two extra passes; five passes
  ran.
- 15 pairs each of `ignite check` and `ignite plan` of the design file.
- Three pairs of todod builds per target and profile.
- A pair is dropped when its two start loads differ by more than 5 or either exit is not 0.
- Ratios are after / before, the median of the pair ratios.
- The units 19.17 adds are timed by `scripts/extra.sh` (`records/extra-steps.txt`, one `STEP`
  line per run: unit, side, run, seconds, start load, exit):
  - `ignite check examples/todo/todo.ignite`, 15 interleaved pairs;
  - `ignite conformance` per protocol and target, three cold runs (fresh `--out` and
    `--target-dir`, `SCCACHE_RECACHE=1`) and three warm runs in the same dirs, after only;
  - `ignite rfc import docs/rfc/rfc9110.xml` with D4's sections, five runs, after only.

## Results

| unit | before s | after s | ratio | pairs kept/dropped |
|---|---|---|---|---|
| gate, warm after the touch | 395.39 | 393.15 | 1.02 | 3/2 |
| `cargo test --workspace`, warm | 403.41 | 401.10 | 0.99 | 4/1 |
| fmt, warm | 0.967 | 0.798 | 0.82 | 3/2 |
| clippy, warm | 4.384 | 4.035 | 1.00 | 3/2 |
| `task test-ignite`, warm | 1.257 | 1.425 | 1.16 | 5/0 |
| `--no-run`, warm / after the touch | 0.032 / 2.987 | 0.030 / 4.025 | 0.93 / 1.13 | 5/0 |
| `ignite_test::` file | 2.135 | 2.363 | 1.12 | 5/0 |
| `ignite-rustgen` integration tests | 1.825 | 2.429 | 1.33 | 5/0 |
| todod Rust dev / release | 3.468 / 4.537 | 3.449 / 4.582 | 0.97 / 0.98 | 3/0 |
| todod LLVM dev / release | 0.293 / 0.752 | 0.340 / 0.866 | 1.14 / 1.15 | 3/0 |
| `ignite check`, `ignite plan` of the design file | 0.079, 0.077 | 0.089, 0.084 | 1.12, 1.09 | 15/0 each |
| `ignite check` of the todo example | 0.0370 | 0.0496 | 1.35 (1.15 to 1.41) | 15/0, loads 8.8 to 9.4 |
| `task test-ignite`, cold | 14.289 | 12.187 | 0.98 | 2/0 |
| gate, cold | 551.68 (pair 1), 588.20 (pair 2, failed) | 448.03, 543.76 | none valid | 0/2 |

After only, medians of three runs at loads 8.2 to 10.4. Every `ignite conformance` run exits 1,
because the claims do not hold (19.0 rows 2 and 3):

| unit | cold s | warm s |
|---|---|---|
| `ignite conformance std/protocol/telnet.ignite --target rust` | 4.545 | 0.546 |
| same, `--target llvm` | 0.805 | 0.554 |
| `ignite conformance std/protocol/http.ignite --target rust` | 5.193 | 0.385 |
| same, `--target llvm` | 0.651 | 0.361 |
| `ignite rfc import docs/rfc/rfc9110.xml --sections 5,6,7.2,9.1,9.2,9.3.1,9.3.2,9.3.3,9.3.5`, 5 runs | 0.177 (0.168 to 0.208) | |

The import differs from the committed `std/rfc/rfc9110.ignite` only by the hand review of its
`party` lines (19.0 row 30).

- **Cold gate:**
  - Pair 1 was dropped by the load rule (12.7 against 5.4).
  - In the retry, the before tree's `cargo test --workspace` failed
    `limits::effect_inference_is_linear_in_either_order` (`crates/ignite-check/tests/limits.rs:48`,
    a timing bound; `logs/baseline-cold-2-workspace-failed.log`).
  - The retry's loads also differ by more than 5 (6.1 against 15.6), so no cold unit has a
    valid pair (`records/summary.md`, "Missing evidence").
  - The after tree passed both cold gates.
- **Test counts in the warm passes:**
  - before: 819 cargo tests passed, 6 ignored; 191 inline per target;
  - after: 921 passed, 13 ignored; 199 inline per target.
- **The merge gate:**
  - `task gate` at `621070e` (`logs/merge-gate-621070e.summary`): 2026-11-04 01:37:59 to
    01:45:26 UTC, 447 s, exit 0, start load 11.6;
  - 923 cargo tests passed, 0 failed, 13 ignored; 199 inline per target;
  - the `crates/ignite` end-to-end binary: 310 passed, 8 ignored, 430.29 s.

## Rises over 10 %

Measured, `git diff --shortstat 296f849 c20f04b`:

- `std/`: 8 files, +3,407 lines. The five catalogues (`std/rfc/` and `std/rfc.ignite`) are
  3,108 of them. `http.ignite` grew from 885 to 1,003 lines and `telnet.ignite` from 1,086 to
  1,195.
- `crates/*/src`: 21 files, +3,954 lines.
- `crates/*/tests`: 100 files, +7,786 lines.

Causes, each inferred unless it says measured:

- **`ignite check` of the todo example, 1.35; of the design file, 1.12; `ignite plan`, 1.09:**
  - `std.protocol.http` imports the RFC 9112 and 9110 catalogues, so every program that reaches
    it loads and types them. The design file reaches `telnet.ignite`, which imports RFC 854 and
    RFC 855.
  - This is the expected cost 19.17 names.
  - Not split by phase.
- **todod LLVM dev 1.14 and release 1.15:**
  - The same frontend cost on a build of 0.3 to 0.9 s.
  - The catalogue constants are unreferenced and leave no definition in `std.ll` (19.0 row 5,
    `conformance::emitted_programs_carry_no_catalogue`, passes).
  - Rust dev and release do not rise.
- **`ignite-rustgen` integration tests, 1.33:** the test helper loads and types the full std per
  program (slice 12b's finding), and std grew by the catalogues.
- **`task test-ignite` warm, 1.16:**
  - Measured: inline tests rose from 191 to 199 per target, and test execution stayed at
    0.34 s per target.
  - The rise is therefore the frontend and the compile of the larger test program; the catalogue
    constants are compiled into the test executables of `telnet.ignite` and `http.ignite`.
- **`--no-run` after the touch, 1.13:**
  - The crates that depend on `ignite-check` grew: `crates/ignite/src/rfc/`,
    `crates/ignite/src/conformance.rs` and the clause handling in syntax, check, types and eval.
  - Their test files grew too.
  - Inferred from the source growth, not split.
- **`ignite_test::`, 1.12:** each test spawns `ignite test` on std programs, which pay the larger
  frontend.

No baseline moved. The apply suites stay the slice-13 exception. The warm gate is at a ratio of
1.02 because the apply suites dominate it.

## Files

| file | content |
|---|---|
| `records/records.jsonl`, `run.json`, `summary.json`, `summary.md` | the `ignite-perf` records (`ignite-performance/1`). `summary.json` says `accepted: false`, because no published comparison was configured, the before tree is not the frozen 11b tree and the cold pairs are invalid. This slice uses the records as measurements, not as 11b acceptance |
| `records/extra-steps.txt` | the `STEP` lines of `scripts/extra.sh` |
| `logs/baseline-cold-2-workspace-failed.log` | the before tree's failed cold `cargo test --workspace` |
| `logs/gate-c20f04b.summary`, `logs/merge-gate-621070e.summary` | the result lines of `task gate` at `c20f04b` (2026-11-04 00:09 to 00:17 local, exit 0, 921 passed, 13 ignored) and at `621070e` |
| `scripts/perf.sh`, `extra.sh`, `final.sh` | the invocations, as run (recorded evidence) |
