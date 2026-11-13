---
format: aep.planning-md/3
id: review-result:adversary-path-cost-1-pass-1
kind: review-result
status: active
title: Adversary pass 1, unit pathcost-1 (wave 15)
relations:
- reviews: story:path-cost-counted-at-every-merge
revision: 1
---
red

Adversary pass on unit `pathcost-1` (the first part of `story:path-cost-counted-at-every-merge`) at `45a093f0` (2026-11-12): NEEDS-CHANGE; 13 findings, all introduced. Tests in `tools/ignite-perf/tests/path_cost_adv.rs` (`0119dcc3`), 12 cases, 11 red at `45a093f0`.

Fixed in `b28c42f6`:

- Blocker: an unchanged tree was refused in 1 of 6 passes of `task path-cost` while another process ran on the same CPUs. A cell above its band is now measured up to twice more and refused only when every measurement is above. Ten passes in a row, five of them with a second run beside, gave no refusal and no second measurement.
- The uniform band of 1 % passed one extra allocation per request. The band of a cell is twice the spread of its recorded runs, at least 1 ‰ of the instructions (1 % and one call of the system calls) and at most a tenth; a rise of 2,464 instructions on `rust/todo/patch` is refused.
- A cell whose recorded runs spread widely was not judged at all, and one hand-edited run switched a cell's judgement off. Every cell is judged and printed with its band and what sets it.
- The frozen check did not cover the scenario script. The counts files hold a SHA-256 of the script and of each program file; counts of a release without a `frozen` line fail.
- SIGTERM to the mode left its valgrind server running. Every child has its own process group, and SIGINT, SIGTERM and SIGHUP end those groups; no new `unsafe`.
- A system call replaced by another at the same count passed unnamed; a new call made at least once per execution is refused and named.
- `file` names with `..`, `/` or an absolute path, a count above 10^15, duplicate cells, a value that is not the median of its runs, and `--record first` on a frozen scenario are refused.
- Two runs in one work directory deleted each other's files; `<work>/lock` refuses the second.
- The work directory's path moved the counts by up to 280 instructions. The servers are staged, built and run from a directory named by a digest of the work directory, of one length in every tree.
- The docs' process count named 24 of 65 processes; the tally names every kind.

Open after the fix pass, stated by the implementor as unknown:

- Whether two trees with paths of different length give the same counts: the binaries still embed paths of the tree. Two work directories 120 characters apart gave identical server cells.
- The counts depend on glibc, the Rust and LLVM versions and the CPU, with no pinned toolchain (the second part of the story).
- The server has a second steady state 13 to 280 instructions below the first; it is inside every band and its cause is not known.

The adversary case `adv_the_values_of_an_unchanged_tree_are_not_refused` was rewritten in the fix pass for the new rule: the values of the six refused passes, as distances from the recording, are the first measurement, and the measurements taken again return the recorded value. The other eleven cases are unchanged.

Eleven mutants of the fix pass, one per guard, each fail a test. The coordinator reran the suite on the branch merged with `main`; the result is in the story's evidence.

```findings
[
{"file":"tools/ignite-perf/src/path_cost.rs","line":1075,"category":"acceptance","severity":"blocker","verdict":"NEEDS-CHANGE","origin":"introduced","message":"An unchanged tree was refused in 1 of 6 passes: llvm/terminal/session/syscalls 167 (band 1) and rust/todo/connection/instructions 213697 (band 2083) are judged although their real spread exceeds the band."},
{"file":"tools/ignite-perf/src/path_cost.rs","line":48,"category":"acceptance","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"The uniform 1 % band (8,005 on rust/todo/patch whose spread is 195) passes one extra allocation (2,464) or body copy (3,338 measured) per PATCH request."},
{"file":"tools/ignite-perf/src/path_cost.rs","line":543,"category":"acceptance","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"The frozen check never compares the scenario script, so repeat, requests or a dropped frozen line pass while the counts stay judged."},
{"file":"tools/ignite-perf/src/path_cost.rs","line":1067,"category":"acceptance","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"A not-judged cell has no upper bound (150 % passes), and one hand-edited run un-judges any cell."},
{"file":"tools/ignite-perf/src/path_cost.rs","line":1150,"category":"judgement","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"--record takes any v* label or first on a frozen scenario, parse_counts accepts non-median values and duplicates, and against_main is not applied to path-cost, so counts and bars can be raised in one merge without a decision."},
{"file":"tools/ignite-perf/src/path_cost_run.rs","line":85,"category":"concurrency","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"SIGTERM to ignite-perf leaves its valgrind server running, reparented to PID 1."},
{"file":"tools/ignite-perf/src/path_cost.rs","line":1063,"category":"acceptance","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"A system call replaced by a costlier one at the same count passes unnamed."},
{"file":"tools/ignite-perf/src/path_cost.rs","line":269,"category":"boundary","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"file names with .., / or an absolute path are accepted and staged by Path::join."},
{"file":"tools/ignite-perf/src/path_cost.rs","line":73,"category":"boundary","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"A count near u64::MAX panics with multiply overflow in the debug build the gate runs; only a hand-edited file reaches it."},
{"file":"docs/quality.md","line":378,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"One pass starts 24 processes omits 18 vgdb runs, about 23 git runs and every child process of the ignite builds."},
{"file":"tools/ignite-perf/src/path_cost_run.rs","line":168,"category":"judgement","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"Counts depend on glibc, Rust, LLVM, the CPU and the work path, with no pinned toolchain and no object names in refusals; not shown to refuse today (unit 2)."},
{"file":"tools/ignite-perf/src/path_cost.rs","line":82,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"Removing the at-least-one-call floor from band survives the suite; adv_one_call_more_on_a_cell_under_100_calls_is_within_the_floor kills it."},
{"file":"tools/ignite-perf/src/path_cost_run.rs","line":136,"category":"concurrency","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"Two runs with one work directory delete each other's runs/, src/ and build/ directories; nothing locks it (read from the code, not run)."}
]
```
