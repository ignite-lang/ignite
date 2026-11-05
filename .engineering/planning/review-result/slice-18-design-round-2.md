---
format: aep.planning-md/3
id: review-result:slice-18-design-round-2
kind: review-result
status: active
title: Design critic, slice 18 round 2
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
approve

**What I read:** the epic and all 9 stories with `aep plan artifact show`, plus `review-result:slice-18-design-round-1`. I also ran `aep plan artifact relations`, `graph` (filtered to the set and its outside neighbours), `waves` and `validate` (valid, 90 artifacts). I read `docs/slice.md` 22.0, 22.8, 22.9, 22.13 and 22.15 from `main` at `727ec17da`, after `git fetch origin` and `git pull --ff-only` (the fetch printed a git lock-file warning, and the pull then updated `main`).

**Shape:**
- **Edges:** I walked about 30 `depends_on` and `decomposes` edges. This included the outside neighbours `slice-17-contract`, `slice-17-records`, `slice-13-recovery-gaps`, `playground-substrate` and `slice-13-local-apply`. There is no cycle.
- **Not a queue:** the waves are W1 U0; W2 U1, U2 and U3 in parallel; W3 U4 and U5; W4 U6 and U7; W5 U8. The longest path is U0, U1, U4, U6, U8.
- **Acceptance reads:** I traced 20 reads to a producer in the set, and an edge records every one.
  - U4 reads U1's outcome type and U2's `hosts` edges.
  - U5 reads U3's `inspect` and the host journal, and U1's terminal design.
  - U6 rows 20 to 22 read U3's repair record, anchor and `journal.intact` report. This is round 1's finding, and the `depends_on story:host-journal-integrity` edge is now there.
  - U6 rows 16 to 19 read the todo (U4).
  - U7 reads `health.ignite` (U4).
  - U8 reads the U6 and U7 records.
- **Round 1 finding:** it is fixed. The edge is present, and `story:apply-recovery-way-out` now names U5, its rows 25 and 26 and its first failing test.
- **Edge U6 to U5:** no row of 22.0 from 16 to 22 reads `ignite recover`. The edge exists because W3 and W4 are the only waves that add a `mod` line to `crates/ignite/tests/main.rs`, and 22.13 states that order. It is a shared-file ordering edge, not a split abstraction, and it does not make the set a chain.
- **Cuts:** each story is demonstrable through its own rows. U1 works alone on the Telnet outcomes (22.14), U2 through `plan::store_is_hosted_by_its_server`, and U3 through its unit tests. U6 is an integration unit, like `story:slice-13-acceptance-matrix`. No layer cut.

**What I could not establish:**
- Out of my lane (scope): 22.13 gives U8 "README and `site/` wording (S10, S11)", while `story:slice-18-records` lists S11 as out of scope and names neither README nor `site/` in its scope.
- Out of my lane (parallel safety): the `crates/ignite/tests/plan.rs` collision between U1 and U2. The contract states the merge order.
- Risk, not a shape finding: F11, that a todo rollout leaves the `Store` members unchanged, is "inferred from the code". If row 17 shows otherwise, no unit owns the repair in `apply/plan.rs`.

```findings
[]
```
