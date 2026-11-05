---
format: aep.planning-md/3
id: review-result:slice-18-acceptance-round-2
kind: review-result
status: active
title: Acceptance critic, slice 18 round 2
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

story:slice-18-records — the acceptance "`STATUS.md`, `PLAN.md` and `docs/vision.md` (S1, S6, S12) each name slice 18 with its merge commit" cannot be met by this unit, because U8 is the last unit on the slice branch and the merge commit does not exist until after U8 lands. The PLAN.md rows of slices 15 to 17 read "merge into `main` pending" and name the gate commit instead. — .engineering/planning/story/slice-18-records.md:45
story:slice-18-records — the Outcome promises that `AGENTS.md` says what slice 18 landed, but the acceptance checks only the gate-time paragraph (row 32). S10 (the layout and commands rows for `ignite recover`, `examples/todo/health.ignite`, the durable todo, `Sample` and `Verdict`) has no checkable sentence. — .engineering/planning/story/slice-18-records.md:29
story:apply-cycle-timing — "`ignite-perf self-check` exits 0 with a case for the mode" reads the same before the work as after it. Self-check exits 0 today and prints one fixed line (`tools/ignite-perf/src/perf.rs:1995`), so "a case for the mode" cannot be observed. — .engineering/planning/story/apply-cycle-timing.md:36
story:apply-cycle-timing — "run on one tree it writes … `apply-cycle-todo-<target>`" contradicts 22.15 and D12. The todo cannot be applied at the base tree, so the todo unit exists only on the after tree, and the sentence does not say which tree it checks. — .engineering/planning/story/apply-cycle-timing.md:33
story:slice-18-contract — the acceptance "a command and an exact result per row" fails for row 32. Its command column is the bare "22.15" and its observation is "the records of 22.15 exist under `docs/performance/18/` and in `AGENTS.md`", which names no file, command or text to compare. — .engineering/planning/story/slice-18-contract.md:41

What I read: 9 of 9 stories with `aep plan artifact show` (whole bodies), `review-result:slice-18-acceptance-round-1`, and `docs/slice.md` section 22 (22.0 to 22.17) on main at 727ec17da after `git fetch` and `git pull --ff-only`. I also checked the repository:
- E150 and E151 are in section 8 (`docs/slice.md:998`).
- `docs/slice18-review.md` exists.
- Eight test names cited by 22.0 grep to existing tests.
- `PLAN.md` rows 15 to 17 were read for how earlier slices name their merge.
- Self-check was read at `tools/ignite-perf/src/perf.rs:1936`.

Round 1 against this round: the `slice-18-contract` acceptance is now five bullets, and it names the W2 shared file `plan.rs` with its merge order, so both round-1 findings on it are fixed. The `apply-recovery-way-out` acceptance now cites rows 25 and 26, S8 and rows 8 and 31, and splits the outcomes. `slice-18-records` no longer reads "none (records)". `apply-cycle-timing` now states a before and an after, with the rows 8 and 31 hold. Two round-1 findings are partly open: the S10 half of the records finding (now finding 2) and the apply-cycle self-check clause (now finding 3).

What I could not establish:
- Whether `Mode::ApplyCycle` or a per-mode self-check case can be selected or listed. The output is one fixed line, so I could not tell what observable the fix should name.
- Row 30 and 22.15 use `<base>` and "the slice base". The text of 22.0 still says slice 17 is "not on main yet" (`docs/slice.md:7431`), although it merged at 421742b33. I judge this unease, not a finding, because the contract story's acceptance does not test it, but the text should be corrected.
- The records story's Out of Scope drops S11, yet 22.11 S11 and 22.13 U8 give it to U8. That is a scope and design question, not acceptance.
- `store-hosted` has the typo "Rows 10", which is style only.

```findings
- file: .engineering/planning/story/slice-18-records.md
  line: 45
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the acceptance requires STATUS.md, PLAN.md and docs/vision.md to name slice 18 with its merge commit, but U8 is the last unit on the slice branch so that commit does not exist when the unit lands; earlier slices' PLAN.md rows say 'merge into main pending' and name the gate commit"
- file: .engineering/planning/story/slice-18-records.md
  line: 29
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome promises AGENTS.md says what slice 18 landed, but the acceptance checks only the gate-time paragraph (row 32), so S10 (layout and commands rows for ignite recover, examples/todo/health.ignite, the durable todo, Sample and Verdict) has no checkable sentence"
- file: .engineering/planning/story/apply-cycle-timing.md
  line: 36
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "'ignite-perf self-check exits 0 with a case for the mode' reads the same before and after the work: self-check exits 0 today and prints one fixed line, so 'a case for the mode' cannot be observed"
- file: .engineering/planning/story/apply-cycle-timing.md
  line: 33
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "'run on one tree it writes ... apply-cycle-todo-<target>' contradicts 22.15 and D12, where the todo cannot be applied at the base tree and its unit exists on the after tree only, and it does not say which tree the check is made on"
- file: .engineering/planning/story/slice-18-contract.md
  line: 41
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the acceptance 'a command and an exact result per row' fails for row 32 of 22.0, whose command is the bare '22.15' and whose observation 'the records of 22.15 exist under docs/performance/18/ and in AGENTS.md' names no file, command or text to compare"
```
