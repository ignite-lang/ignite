---
format: aep.planning-md/3
id: story:path-cost-counted-at-every-merge
kind: story
status: active
title: The cost of an existing path is counted at every merge
relations:
- informed_by: story:release-refuses-every-worse-row
- informed_by: story:size-rows-grow-with-a-decision
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-11T13:47:21Z", actor: "human:timo", revision: 2, executor: "agent:claude-coordinator", correlation: "accepted-2026-11-11"}
- {from: "proposed", to: "active", at: "2026-11-11T13:47:21Z", actor: "human:timo", revision: 3, executor: "agent:claude-coordinator", correlation: "accepted-2026-11-11"}
---
# Story: the cost of an existing path is counted at every merge

## Outcome

Every merge measures, in instructions and system calls, a fixed set of scenarios whose programs are frozen at the last release, and is refused when one of them costs more than at that release. A new feature brings its own scenario, whose first value is recorded without a comparison. The reader of a refusal sees which functions and which system calls grew.

## Why

Timo, 2026-11-11: "if we're adding functionality, then of course it will have additional system calls around ... But how do we make sure these additional system calls are actually there because we're adding new features ... or whether they might have been accidentally added to existing path of feature execution and therefore slowing down things which people expect to be fast already." And: a release check may run, "just not one which is like putting additional heavy load on a machine which already is heavily under load".

What the gate does today, read on `5b761b3e`:

- No row of `docs/quality.md` measures the cost of a path in a deterministic unit. `Taskfile.yml` and `tools/ignite-perf/src` name neither `callgrind` nor `strace`. The start, per-request, per-connection and per-session counts of wave 11 (for example LLVM instructions per connection 22,022 at v0.15.0 and 35,506 now; `terminald` session system calls 25 and 63; `ignite check` 2,678 system calls) were measured by adversaries by hand.
- The cost of an existing path is judged only as time, by rows 3.3, 3.7, 3.8, 5.3, 6.1 and 6.2, on the dedicated host, once per release. Waves 9 and 10 merged without that run; the first v0.16.0 candidate was refused on 14 cells of these rows, and wave 11 (11 stories, no feature) is the repair.
- Rows that are totals over all code (3.2 and 3.9 binary size) grow when a release adds code; `story:size-rows-grow-with-a-decision` changed their rule on the same day.
- The release check is a cold `task gate`, `task quality` and `task quality-remote`; the first two run on the development machine.

Coordinator's reading, not Timo's words:

- A scenario frozen at release N cannot use a feature added after N. So a count that rises on it is, by construction, cost added to an existing path; a count that belongs to a new feature shows up only in that feature's own scenario. This is the separation the question asks for.
- Counts are the per-merge instrument because they are deterministic (run to run ±30 instructions in the adversaries' runs), need one core and no load, and so are allowed on the development machine. Time stays the judge of a release and stays on the dedicated host.
- A change of design can raise counts and lower time (the readiness scheduler replaced a thread per connection, whose cost v0.15.0 paid in the kernel). Such a merge passes with a `decision` that cites a dedicated-host record showing the scenario's time rows not worse.

## Acceptance

- A directory of scenarios, each a frozen program source plus a script of operations, with the release it was frozen at. The first set is frozen at v0.16.0 and covers what the wave-11 adversaries measured: start of `todod` and `terminald`; one GET, PATCH and DELETE of the todo; one connection opened and closed; one `terminald` session; `ignite check`, `ignite plan` and a dev build of the frozen design file. Both targets where the scenario builds on both.
- A row of `docs/quality.md` (name its id) with one cell per scenario, operation, target and unit (instructions, system calls). `task gate` measures it; the instrument is part of `ignite-perf`, not a hand-run script. The measurement uses at most two cores and no load generator; its wall time on the development machine is recorded once in the story's evidence.
- A cell above the value at the last release beyond a stated noise band is refused at the merge, not at the release. The refusal prints the functions whose instruction count grew, the functions that are new, and the system calls whose count grew, each with both values.
- A refused cell passes only with a `decision` on its bar that cites a `host: ec2:*` record in which the time rows of that scenario are not worse than at the last release; the release check lists every such cell with its decision, as it does for size rows.
- A scenario added after the freeze is recorded with its first value and compared from the next release on. A frozen scenario is never edited; a test fails when a file of a frozen scenario differs from the tag it names.
- The compile rows report the instructions of the build per 1,000 lines of compiled source beside the absolute value, so growth of the standard library is told apart from a slower compiler (Timo: "compile time, relative to the file size").
- The cold `task gate` of a release can run on the dedicated host that `task quality-remote` already starts, so the release check adds no build to the development machine; the per-merge gate is unchanged apart from the new row.
- `AGENTS.md` § Releases and `docs/quality.md` state the two rules side by side: a frozen scenario may not cost more; a total over all code may grow with a decision that names the code.
- `task gate` green; no row of `docs/quality.md` worse.

## Not in this story

- The hygiene totals (1.3, 3.6, 7.2, 7.3) keep their rule.
- No change to the rule that a time row worse than the last release refuses the release.

## Scope

- `tools/ignite-perf/src` (the instrument), a new scenario directory under `bench/`, `crates/ignite/tests/quality.rs`, `quality/baseline.json`, `docs/quality.md`, `AGENTS.md`, `Taskfile.yml`, `tools/ignite-perf/src/remote.rs` (the gate on the dedicated host). Inferred.

## Second part

The first part is released with v0.16.3: `task path-cost` in `task gate`, the frozen scenarios, the counts of the last release, the bands, the frozen check (`docs/quality.md` "The path cost at every merge"). The second part was prepared on 2026-11-12 and is not started; no code exists for it.

Bullets of the Acceptance that remain:

1. **The decision (bullet 4).** A refused cell passes only with a `decision` on its bar in `quality/baseline.json` that cites a `host: ec2:*` record (`quality/history.jsonl`, the run id) in which the time rows of that scenario are not worse than at the last release. The size rows are the shape to follow (`tools/ignite-perf/src/remote.rs`; the tests `a_grown_cell_of_a_size_row_without_a_decision_is_refused`, `a_decision_on_a_size_cell_not_above_the_last_release_is_refused`, `a_grown_cell_held_by_the_last_releases_decision_is_refused` in `crates/ignite/tests/quality.rs`): the decision names the cell, the value it accepts, the run id and the story; a decision on a cell that is not above the release's count is refused; a decision whose run id is not in the history, is not `ec2:*`, or whose time rows for that scenario are worse, is refused; a decision carried over from the last release does not hold a new rise. `judge` in `tools/ignite-perf/src/path_cost.rs` is where a decision attaches. The docs state which time rows belong to which scenario (`todo`: the 3.7 and 3.8 rows of `todod`; `terminal`: the session rows; `design`: the 6.x build rows), held in one table in the code.
2. **Compile rows per 1,000 lines (bullet 6).** The cells of the `design` scenario report instructions per 1,000 lines of compiled source beside the absolute value; the lines are those of every source file the compiler read for the command, counted by the compiler or the instrument. The refusal judges the per-1,000-lines value for these cells and prints both.
3. **The cold gate on the dedicated host (bullet 7).** `ignite-perf quality-remote` gains a way to run `task gate` cold on the host it already starts and to bring back its result (exit code, result lines, the path-cost metrics). Reuse the launch, the upload of the tree and the cost record of `tools/ignite-perf/src/remote.rs`; no new AWS resource type; no credential or profile name in any file. The mode, its argument tests and a dry-run test that prints the commands it would send come first; the first real run is the coordinator's and needs the operator's word on its cost.
4. **The two rules side by side (bullet 8)** in `AGENTS.md` § Releases and `docs/quality.md`: a frozen scenario may not cost more; a total over all code may grow with a decision that names the code.

Handed over from the adversary pass on the first part:

- **The counts and bars can be raised without a decision.** `--record` accepts any `v*` label from any commit; re-recording plus `IGNITE_BLESS=1` raises the bars of 3.10 in one merge and nothing refuses it (`against_main` is never called for the source `path-cost`). To do: `--record vX.Y.Z` runs only on the tree of that tag; a bar of 3.10 above `main`'s needs the decision of bullet 1; the release steps in `AGENTS.md` § Releases name `task path-cost RECORD=`.
- **The counts depend on the machine.** Shares of the judged cells: start cells `ld-linux` 67 to 86 %, libc 5 to 28 %; request cells libc 15 to 44 %, Rust std 16 to 31 %; `design` check and plan Rust std 55 %; `llvm/design/build` ld-linux 12.7 %, libLLVM 11.3 %, lld 11.2 %. There is no `rust-toolchain` file. An update of glibc, Rust or LLVM, or another CPU and distribution, likely refuses merges that change no code, and the refusal's function names carry no object. To do: the `.counts` file names what it was recorded with (glibc, rustc, clang, valgrind, CPU model); when the machine differs from the recording, the mode says so before it judges and either measures the release again on this machine or refuses with the command that does; function lines carry their object.

Found while v0.16.3 was cut (2026-11-13):

- The release recorded the counts of v0.16.3 with three cells above v0.16.2 (story `path-cost-cells-above-v0-16-2`). At a release the rule "tracked, not refused" was applied to row 3.10; the row has no decision mechanism yet (bullet 1).
- A server without `IGNITE_WORKERS` reads the caller's cgroup files, which moved the four `start` cells by 13 system calls and about 7,000 instructions between a session scope and the build pool; the servers now start with `IGNITE_WORKERS=2`. One system call of the `start` cells still differs between the build pool (76, 100, 80, 104) and outside it (75, 99, 79, 103); the cause is not known. The counts are recorded under the build pool.
- `llvm/terminal/session` read 245,441 and 244,745 in two recordings of v0.16.2, each without spread; the cause is not known.
- Whether two trees with paths of different length give the same counts is not known: the binaries embed paths of the tree.
- `IGNITE_BLESS=1` on the three page tests at once (`quality::the_path_cost_bars_are_the_recorded_counts`, `quality::trend_and_docs_are_generated_from_quality`, `site::quality_page_is_generated`) failed once: the first writes the bars the second reads. With `--test-threads=1` they pass.

Constraints: a bar of 3.10 does not move and the counts are not re-recorded by this unit; a change to the judgement gets a test with a fixture pair (`tools/ignite-perf/tests/path_cost.rs`, `tests/fixtures/path-cost/`); no test needs the network or AWS.
