---
format: aep.planning-md/3
id: review-result:slice-18-scope-round-1
kind: review-result
status: active
title: Scope critic, slice 18 round 1
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

- story:slice-18-fault-matrix — the outcome omits "an `Availability` contract over them reaches each `Status`" (epic promise, run under `ignite run` with a manual clock). Row 16 is the only check of it and U6 is the only unit that cites row 16, but only by number in the acceptance. A reader marking the story done on its outcome, or on its title ("passes the fault matrix"), can drop the promise unseen. — `.engineering/planning/story/slice-18-fault-matrix.md:25` (promise at `.engineering/planning/epic/slice-18-one-service-operated.md:44-47`; row 16 at `origin/feat/slice18-one-service:docs/slice.md:7481`)
- story:slice-18-fault-matrix — the outcome says "on both targets", but the contract runs rows 17 to 22 on LLVM only and the epic names no target for the fault matrix, so the story claims more than either. — `.engineering/planning/story/slice-18-fault-matrix.md:25` (contract: `origin/feat/slice18-one-service:docs/slice.md:7873`, "Rows 17 to 22 run on LLVM only")
- story:apply-recovery-way-out — its acceptance cites neither rows 25 and 26 of 22.0 nor `ignite recover`. No story in the set claims the refusal of row 26 ("recover refuses what it cannot prove", the epic's "never repeats an effect it cannot prove undone"), or the `abandoned` receipt state of row 25. The story was not re-scoped to U5 when the contract folded it in. — `.engineering/planning/story/apply-recovery-way-out.md:38` (contract U5 row `origin/feat/slice18-one-service:docs/slice.md:7983`; rows 25 and 26 at `:7490-7491`)
- story:slice-18-records — S11 (rewording README and `site/` sentences about what a check or a build proves, review R6) is assigned to U8. No sentence of the epic asks for it: the epic's sources are R2, R10, R11, R13 and its Out of Scope does not cover it. The same unit's `docs/vision.md` and `AGENTS.md` updates trace to the epic. — `.engineering/planning/story/slice-18-records.md:31` (S11 at `origin/feat/slice18-one-service:docs/slice.md:7950`; U8 row `:7986`; epic sources `.engineering/planning/epic/slice-18-one-service-operated.md:31-32`)

**What you read.** I read 11 artifacts (the epic and the nine stories, plus `docs/slice.md` section 22, 667 lines) and ran `aep plan artifact show` on each of the 10 artifacts, `aep plan artifact graph`, `git fetch origin`, and `git show origin/feat/slice18-one-service:docs/slice.md` from line 7424. I extracted 8 promises from the epic: the outcome sentence (durability; probe feeds `Availability`; apply never reports unobserved success or repeats an unprovable effect; LLVM build fraction) and its 4 acceptance bullets. 8 of 8 trace to at least one item. Promise 2 (the contract reaches each `Status`) traces only through row 16 in U6's acceptance, which is the first finding.

**What you could not establish.**
- Whether `story:apply-recovery-way-out` is reach beyond the epic. The epic body never mentions an operator recovery command, but the story was filed with its `decomposes` edge to this epic (commit 2468748d), so I treated it as a deliberate inclusion and raised no finding.
- Whether `tools/ignite-perf` mode `programs` needs a change to time `todod` and `health` for row 27. U7 owns only the new mode `apply-cycle`, and no story visibly owns a `programs` change. This is design and parallel-safety territory, out of my lane, so it set no verdict.
- U8's story scope omits `README`, which 22.13 gives it. That is a scope-field accuracy question for the design critic, out of my lane.
- Row 16 is U6's but its first failing test is the `llvm::` rollout test. That touches how checkable the acceptance is, which belongs to the acceptance critic.

```findings
- file: .engineering/planning/story/slice-18-fault-matrix.md
  line: 25
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the outcome omits the epic promise that an Availability contract over the probe reaches each Status under ignite run with a manual clock (row 16, which only this story's acceptance cites), so a reader marking the story done on its outcome drops that promise"
- file: .engineering/planning/story/slice-18-fault-matrix.md
  line: 25
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the outcome says the fault matrix passes on both targets, but the contract runs rows 17 to 22 on LLVM only (docs/slice.md 22.8) and the epic names no target for it"
- file: .engineering/planning/story/apply-recovery-way-out.md
  line: 38
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the acceptance cites neither rows 25 and 26 of 22.0 nor ignite recover, so no story claims the refusal of row 26 (never repeat an effect it cannot prove undone) or the abandoned receipt state; the story was not re-scoped to unit U5 of 22.13"
- file: .engineering/planning/story/slice-18-records.md
  line: 31
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "unit U8 owns S11, rewording README and site sentences about what a check or build proves (review R6), which no sentence of the epic asks for"
```
