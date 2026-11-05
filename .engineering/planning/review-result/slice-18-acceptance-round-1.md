---
format: aep.planning-md/3
id: review-result:slice-18-acceptance-round-1
kind: review-result
status: active
title: Acceptance critic, slice 18 round 1
relations:
- reviews: story:slice-18-contract
- reviews: story:typed-probe-outcomes
- reviews: story:store-hosted
- reviews: story:host-journal-integrity
- reviews: story:durable-todo-service
- reviews: story:apply-recovery-way-out
- reviews: story:slice-18-fault-matrix
- reviews: story:apply-cycle-timing
- reviews: story:slice-18-records
revision: 1
---
needs-revision

story:slice-18-contract — the acceptance joins five independent outcomes (table rows, non-overlapping units, decisions with options, section-8 codes, committed review with blockers folded) in one sentence, so one can pass while another fails — .engineering/planning/story/slice-18-contract.md:41
story:slice-18-contract — the acceptance requires "units whose owned files do not overlap within a wave", but 22.13 W2 has U1 and U2 both editing `crates/ignite/tests/plan.rs` (U2 appends, "or a trivial append conflict"), so the sentence is false against the contract it judges — docs/slice.md@origin/feat/slice18-one-service:7994-7997
story:apply-recovery-way-out — the acceptance names no command, test or exit result: "runs the stated operator command" points at a 17.5 line that does not exist yet, while 22.0 rows 25 and 26 and 22.9 give the exact command (`ignite recover --state S --operation <id>`), exit codes, `operatorRecovered` record and `abandoned` state, and the story cites none of them — .engineering/planning/story/apply-recovery-way-out.md:38
story:apply-recovery-way-out — the acceptance holds two independent outcomes (an e2e test applies a plan after recovery; the journal records the operator's step) and omits rows 8 and 31 and the 17.5 line (S8) that its own Outcome promises — .engineering/planning/story/apply-recovery-way-out.md:38
story:apply-cycle-timing — the first failing test "`ignite-perf self-check naming apply-cycle`" is not a test or a command with an exact result, and the contract gives no exact observation for it (22.13 U7 row column: "28 (mode)"), so red-before and green-after cannot be checked the same way twice — .engineering/planning/story/apply-cycle-timing.md:29
story:apply-cycle-timing — "Rows 28 (mode)" has no exact result to pass: row 28's observation is about records for both targets and trees and the todo median, which U8 produces ("U7 (mode), U8 (records)"), so U7 has no observable check of the mode itself — .engineering/planning/story/apply-cycle-timing.md:30
story:slice-18-records — the first failing test is literally "`none (records)`", which cannot be red before or green after, so the first bullet asserts something that cannot be observed — .engineering/planning/story/slice-18-records.md:40
story:slice-18-records — the Outcome promises "the standing documents say what landed", but rows 27, 28 and 32 only check performance records and the AGENTS.md paragraph, so no row covers the S1, S6, S10, S11 and S12 edits to `STATUS.md`, `PLAN.md`, `docs/vision.md`, README and `site/` — .engineering/planning/story/slice-18-records.md:41

What I read: all nine stories in full with `aep plan artifact show` (9 of 9), the lifecycle of `story`, and `docs/slice.md` section 22 (22.0 to 22.17) at origin/feat/slice18-one-service (19c70564) after `git fetch origin`. I also checked the branch for the section-8 "Codes of slice 18" paragraph and `docs/slice18-review.md`, the main tree for the journal unit-test names and the `ignite-perf` self-check, and the existence of `story:slice-17-records` and `story:slice-13-recovery-gaps`.

What I could not establish:
- Whether `ignite-perf self-check` can name or select a case such as `apply-cycle`: it exists on main (`tools/ignite-perf/src/perf.rs:1936`), but I did not read how it reports cases.
- Whether row 21's and row 22's unit-level checks (stated only as "pass" in row 24) are exact enough: the story's phrase "the unit level of 20 to 22" resolves to row 24's four tests, so I did not file it.
- Every story, apart from the contract and `apply-recovery-way-out`, splits its acceptance into two bullets, a first failing test and a row list, with the hedge "on both targets where it has a target". I judged this a shared template that the 22.0 rows make checkable and did not file it.
- Out of my lane, not filed:
  - `story:slice-18-fault-matrix` has an Outcome that says "on both targets", but 22.8 says rows 17 to 22 run on LLVM only (scope or design).
  - `story:store-hosted` says "Rows 10" (typo).
  - U1 and U2 sharing `plan.rs` is a parallel-safety matter.

```findings
- file: .engineering/planning/story/slice-18-contract.md
  line: 41
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the acceptance joins five independent outcomes (table rows, non-overlapping units, decisions with options, section-8 codes, committed review with blockers folded) in one sentence, so one can pass while another fails"
- file: docs/slice.md@origin/feat/slice18-one-service
  line: 7994
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the contract story's acceptance requires units whose owned files do not overlap within a wave, but 22.13 W2 has U1 and U2 both editing crates/ignite/tests/plan.rs (U2 appends, or a trivial append conflict), so the sentence is false against the contract it judges; reword the acceptance to name the shared-file rule or remove the overlap"
- file: .engineering/planning/story/apply-recovery-way-out.md
  line: 38
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the acceptance names no command, test or exit result: 'runs the stated operator command' points at a 17.5 line that does not exist yet, while 22.0 rows 25 and 26 and 22.9 give the exact command, exit codes, operatorRecovered record and abandoned state, and the story cites none of them"
- file: .engineering/planning/story/apply-recovery-way-out.md
  line: 38
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the acceptance holds two independent outcomes (an e2e test applies a plan after recovery; the journal records the operator's step) and omits rows 8 and 31 and the 17.5 line (S8) that its own Outcome promises"
- file: .engineering/planning/story/apply-cycle-timing.md
  line: 29
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the first failing test 'ignite-perf self-check naming apply-cycle' is not a test or a command with an exact result, and the contract gives no exact observation for it (22.13 U7 row column '28 (mode)'), so red-before and green-after cannot be checked the same way twice"
- file: .engineering/planning/story/apply-cycle-timing.md
  line: 30
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "'Rows 28 (mode)' has no exact result to pass: row 28's observation is about records for both targets and trees and the todo median, which U8 produces ('U7 (mode), U8 (records)'), so U7 has no observable check of the mode itself"
- file: .engineering/planning/story/slice-18-records.md
  line: 40
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the first failing test is literally 'none (records)', which cannot be red before or green after, so the first bullet asserts something that cannot be observed"
- file: .engineering/planning/story/slice-18-records.md
  line: 41
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome promises the standing documents say what landed, but rows 27, 28 and 32 only check performance records and the AGENTS.md paragraph, so no row covers the S1, S6, S10, S11 and S12 edits to STATUS.md, PLAN.md, docs/vision.md, README and site/"
```
