---
format: aep.planning-md/3
id: story:path-cost-counted-at-every-merge
kind: story
status: draft
title: The cost of an existing path is counted at every merge
relations:
- informed_by: story:release-refuses-every-worse-row
- informed_by: story:size-rows-grow-with-a-decision
revision: 1
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
