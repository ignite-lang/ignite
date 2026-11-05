# Slice 17 gate time (`docs/slice.md` 21.14)

Measured 2026-11-05 08:42 to 09:26 local time (UTC+1). 20 cores, shared with other sessions'
builds; start loads 5.1 to 25.8, with a peak of 54 during the before tree's cold gate
(`logs/watch-2.log`). sccache was on. Every command ran through the session's build-slot wrapper
(`CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=line-tables-only`, `RUST_TEST_THREADS=4`,
`CARGO_BUILD_JOBS=8`, `nice -n 10`, `ionice -c3`). The record is in `AGENTS.md`, "Slice 17".

**The run is incomplete.** The harness's `smoke`, `programs`, `cli` and one `cold` attempt ran;
`warm` did not (below). No pair of this run was taken at a load under 5.

## Trees and harness

- **Before:** `99647132`, `main` when U6 merged it: the merge base of the slice at the close.
  Its code is that of `651be04c`, where the slice started (planning-store commits only between
  them).
- **After:** `884d6b19`, the slice branch with U6's code: row 24's test and the `HttpServer`
  hooks (`docs/slice.md` 21.18). The later commits of the slice change documents only.
- `ignite-perf` built at `884d6b19`, run with `--comparison ""` (`records/run.json`).

## What ran

1. `scripts/mode.sh smoke programs cli` with `scripts/watch.sh` beside it (`logs/modes-1.log`):
   each tree's compiler, then todod per target and profile (the harness stops after 3 valid
   pairs, at most 5 passes), then `ignite check` and `ignite plan` of the design file, 15 pairs.
2. `scripts/extra.sh` (`records/extra-steps.txt`, `STEP` lines) with each tree's compiler and
   runtime archives from step 1: `ignite check examples/todo/todo.ignite` (15 pairs); the same
   with the after compiler over two std trees, `ac043ae8`'s (the slice before the `HttpServer`
   hooks) and `884d6b19`'s (15 pairs, to attribute the check's rise); terminald per target and
   profile (3 pairs); after only, the build of `studio.ignite` per target and profile and
   `ignite test std/fs.ignite` and `std/process.ignite` per target, cold and warm (3 runs each).
   Every build has a fresh `--out` and `--target-dir`, cold builds run with
   `SCCACHE_RECACHE=1`. `scripts/pairs.awk` (slice 16's) applies the pair rules and wrote
   `records/extra-pairs.txt`.
3. `scripts/mode.sh cold` (`logs/modes-2.log`): the before tree's cold gate, then the after
   tree's. The watch cleaned the before tree's target once the after tree's cold rows began.
   The two gates' start loads differ by 6.0 (21.8 against 15.8), so the harness started its
   retry; `scripts/stop-retry.sh` stopped it at the retry's clippy step and cleaned both targets,
   on the disk request below.
4. `scripts/mode.sh summarize` wrote `records/summary.md` and `summary.json` (exit 2: not
   accepted, warm evidence missing).
5. `records/emit-todo-std.txt`: the generated `ignite-std` crate of todod with each of the two
   std trees, by `ignite emit`.

## Results

Ratios are medians of pair ratios; a pair is dropped when its start loads differ by more than 5
or an exit is not 0.

| unit | before s | after s | ratio | pairs kept/dropped | start loads |
|---|---|---|---|---|---|
| gate, cold (fmt, clippy, workspace test, test-ignite) | 1070.99 (1.62, 10.15, 1031.21, 28.01) | 810.17 (2.76, 24.37, 763.85, 19.17) | no valid pair (0.76) | 0/1 | 21.8 against 15.8; steps 14.1 to 21.8 and 10.1 to 19.4 |
| `ignite check`, `ignite plan` of the design file | 0.150, 0.144 | 0.177, 0.153 | 1.09, 1.06 | 15/0 each | 16.2 to 16.7 |
| `ignite check examples/todo/todo.ignite` | 0.067 | 0.127 | 1.94 | 15/0 | 9.5 to 9.9 |
| same, after compiler, std before and after the `HttpServer` hooks | 0.068 | 0.131 | 1.89 | 15/0 | 9.5 to 9.9 |
| todod Rust dev / release | 3.916 / 5.145 | 4.324 / 7.096 | 1.14 / 1.42 | 4/0, 3/1 | 5.1 to 22.6 |
| todod LLVM dev / release | 0.383 / 1.368 | 0.476 / 0.999 | 1.24 / 1.05 | 3/1, 4/0 | 7.4 to 22.6 |
| terminald Rust dev / release | 3.915 / 4.615 | 5.866 / 5.556 | 1.50 / 1.19 | 2/1, 3/0 | 9.5 to 19.9 |
| terminald LLVM dev / release | 0.568 / 0.926 | 0.596 / 0.895 | 1.03 / 1.05 | 3/0 each | 9.3 to 21.3 |

The todod rows are the harness's (`records/summary.md`); its absolute columns are its own
medians. Test counts in the cold gates: before 1014 cargo tests passed, 10 ignored; after 1089
passed, 10 ignored.

After only, medians of three runs:

| unit | Rust s | LLVM s | start loads |
|---|---|---|---|
| `studio.ignite` build, dev / release | 5.113 / 7.057 | 0.498 / 0.988 | 9.6 to 21.2 |
| `ignite test std/fs.ignite`, cold / warm (6 tests) | 5.512 / 0.109 | 0.361 / 0.100 | 15.0 to 17.7 |
| `ignite test std/process.ignite`, cold / warm (15 tests) | 8.981 / 0.746 | 0.735 / 0.430 | 17.7 to 22.5 |

The runner's `finished in` for the 15 process tests is 0.36 to 0.95 s (children, sleeps and
timeouts in the tests), for the 6 file tests 0.05 to 0.14 s.

## Rises over 10 %

Measured growth (`git diff --shortstat 99647132 884d6b19`): `std/` +499 lines in 3 files;
`crates/*/src` +4,433 and -213 lines in 38 files, of them `crates/ignite-io/src` +2,201 and -121
in 7 files; `crates/*/tests` +3,867 lines in 67 files.

- **`ignite check` of the todo example, 1.94:** measured to the `HttpServer` hooks. The same
  after compiler takes 0.068 s with the std before the hooks and 0.131 s with them (1.89), and
  the before tree takes 0.067 s, so the capabilities of the slice cost nothing here.
  `std.protocol.http` now imports `std.control` and `std.policy`, which every program that
  imports `std.protocol.http` loads and types; the design file imported them already, which
  is why its check rises 1.09 only. Recorded as `docs/friction.md` row 43 with candidate fixes.
- **todod, Rust release 1.42 and dev 1.14, LLVM dev 1.24:** the generated `ignite-std` crate of
  todod grows from 1,049 to 1,700 lines with the hooks (measured, `records/emit-todo-std.txt`);
  that the record fields `recovery` and `rollout` reach the policy and control types it emits
  is inferred, not split. The LLVM dev rise is the frontend of the check above, inferred. LLVM
  release stays under 10 % (1.05; the absolute medians, 1.368 and 0.999 s, come from different pairs).
- **terminald, Rust dev 1.50 (2 pairs) and release 1.19:** the program does not use HTTP and
  its std is unchanged; every Rust program build compiles `ignite-io`, which grew by 2,201
  lines (`fs`, `child`, `entropy`, the `args` flags) and enables the `nix` features `dir` and
  `poll`. The cause is inferred from that growth, not split; loads were 9.5 to 19.9.
- **Cold gate:** no valid pair. The before tree's gate ran under a load of 14 to 54 and is the
  slower of the two; clippy is 2.40 in the harness's per-step comparison (10.15 against 24.37 s,
  start loads 21.2 and 16.5), inferred to be the larger crates under a different load.

No baseline moved. The apply suites stay the slice-13 exception.

## Uncollected, with the reason

- **`warm`** (gate, `cargo test --workspace`, fmt, clippy, `task test-ignite`, `--no-run`,
  `ignite_test::`, rustgen tests): the interleaved passes need both trees' gate targets built at
  once. The after tree's target was 9.0 GiB at the end of its cold gate (`logs/stop-retry.log`),
  and the machine's disk owner asked during the run (09:22) to keep at most one tree's target
  built at a time and this unit's scratch under 8 GB, with free space at 45 GB. Taking fewer
  passes does not help: one pass already needs both targets.
- **The cold retry:** stopped for the same request before it rebuilt the before tree beside
  the after tree's target.

## Files

| file | content |
|---|---|
| `records/records.jsonl`, `run.json`, `summary.json`, `summary.md`, `tools.json` | the `ignite-perf` records (`ignite-performance/1`) of smoke, programs, cli and the cold attempt |
| `records/extra-steps.txt`, `extra-pairs.txt` | the `STEP` lines of `scripts/extra.sh` and their pairs |
| `records/emit-todo-std.txt` | the generated `ignite-std` line counts of todod with each std |
| `logs/` | mode, watch, retry-stop and summarize logs |
| `scripts/` | the scripts as run; their paths point into the session's scratch directory |
