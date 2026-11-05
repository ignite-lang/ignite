---
format: aep.planning-md/3
id: review-result:slice-18-scope-round-2
kind: review-result
status: active
title: Scope critic, slice 18 round 2
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

- story:slice-18-contract — `docs/slice.md` 22.13 still gives U8 "README and `site/` wording (S10, S11)" and 22.11 S11 still says "reworded to 'states and measures'; U8", although `story:slice-18-records` now lists S11 as out of scope ("no sentence of the epic asks for it; it stays for a later slice"). The contract is what units work from, so U8 would still ship wording that the epic's sources (R2, R10, R11, R13) and Out of Scope do not ask for. This is round 1's S11 finding, fixed in the story but not in the contract. Drop S11 from the U8 row and mark it deferred in the S11 row. — `docs/slice.md:7987` (U8 row), `docs/slice.md:7951` (S11 row), `.engineering/planning/story/slice-18-records.md` (Out of Scope)

**What you read.** I read 11 artifacts and ran `aep plan artifact show` on the epic, the nine stories and `review-result:slice-18-scope-round-1`. I also ran `aep plan artifact graph`, `git fetch origin` and `git pull --ff-only` (already up to date), and read `docs/slice.md` 22.0 to 22.17 (lines 7425 to 8088) on main.

I extracted 9 promises from the epic: 5 clauses of the Outcome sentence and its 4 Acceptance bullets. I traced 9 of 9 to an item:
- **Durability, LLVM:** U4 row 12, U6 row 17 (rollout), with U2 and U3's `workdir` as supporting work.
- **Probe outside `std/`, feeding `Availability`:** U1 (typed `good`), U4 (probe kind, row 15), U6 (row 16, all three `Status` values), row 8 held by every unit.
- **Never reports an unobserved success:** rows 18 and 19, U6.
- **Never repeats an unprovable effect:** U5 row 26, row 18.
- **Fault matrix, three cases:** U3 (rows 20 to 24, 33) and U6 (rows 18 to 22).
- **Timing recorded:** U7 (mode, plus `programs` split) and U8 (rows 27, 28, 32), with the D12 todo-after-only deviation named in the epic and in U8.
- **Fraction of Rust build time:** row 27, U8.

**Round 1 findings:** three are fixed in the set.
- The fault-matrix outcome now carries row 16 and says LLVM only for rows 17 to 22.
- `apply-recovery-way-out`'s acceptance now cites rows 25 and 26 and `ignite recover`.
- `apply-cycle-timing` now claims the `programs` split.
- The S11 finding survives in the contract text, as above.

**What you could not establish.**
- Whether `ignite recover` (U5, `abandoned` receipt state) is reach beyond the epic. The epic names no operator recovery command, and its closest sentences are "never repeats an effect it cannot prove undone" and "a stated recovery or refusal". The story has a `decomposes` edge to this epic and the contract justifies it in 22.9, so I raised no finding, as in round 1. If Timo treats the epic's three fault cases as closed, this is the item to question.
- Out of my lane, did not set the verdict:
  - `story:slice-18-records` does not list `README` in its scope field, though 22.13 gives it to U8 (design).
  - `story:slice-18-contract`'s Context says slice 17 is not yet on main, and 22 opens with the same stale line at `docs/slice.md:7431` (design).
  - Row 16's first failing test is the LLVM rollout test (acceptance).

```findings
- file: docs/slice.md
  line: 7987
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "story:slice-18-contract: 22.13's U8 row and 22.11's S11 row still assign the README and site wording about what a check or build proves (review R6) to U8, which no sentence of the epic asks for and which story:slice-18-records now lists as out of scope, so the contract and the story disagree on whether U8 ships S11"
```
