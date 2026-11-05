---
format: aep.planning-md/3
id: review-result:slice-18-design-round-1
kind: review-result
status: active
title: Design critic, slice 18 round 1
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

- story:slice-18-fault-matrix — its acceptance rows 20 to 22 read `journalRepaired`, `journalRepairs`, the `journal.intact` report and the refusal exits, which story:host-journal-integrity produces. No `depends_on` edge runs to that story. The only path is through story:apply-recovery-way-out, whose rows 25 and 26 the story never reads and which sits there for the shared `main.rs` mod line. Add `depends_on story:host-journal-integrity` to the fault matrix, so the real producer is named and does not hang on an edge that exists for a trivial append conflict. — .engineering/planning/story/slice-18-fault-matrix.md:11, .engineering/planning/story/slice-18-fault-matrix.md:35, docs/slice.md 22.0 rows 20 to 22 and 22.13 U3 row (origin/feat/slice18-one-service:docs/slice.md:7485-7487, 7981)

**What I read:** 9 artifacts plus the epic, each with `aep plan artifact show`. I also ran `aep plan artifact relations`, `graph` (filtered to the set, with the whole graph scanned for edges leaving it), `waves` and `validate` (valid). I read `docs/slice.md` 22.0 to 22.17 from `origin/feat/slice18-one-service` after `git fetch origin`. I walked 28 edges inside the set and its immediate outside neighbours (`slice-17-contract`, `slice-17-records`, `slice-13-recovery-gaps`, `playground-substrate`, `slice-13-local-apply`). I found no cycle. The `depends_on` edges match the 22.13 "depends on" column exactly. The graph is not a serialising chain: W2 has three parallel units and W4 has two. I traced 21 acceptance reads to a producer in the set, and 19 of them have a recorded edge, directly or through a short path. The two without a direct edge are rows 20 to 22 of U6 reading U3, covered by the finding above. The rest of the shape holds. Each story can be shown working through its own rows, and U6 is an integration unit like the implemented `story:slice-13-acceptance-matrix`.

**What I could not establish:**
- F11 is "inferred from the code": a todo rollout replaces `TodoService.server` and leaves the `Store` members unchanged. If row 17 shows otherwise, the repair would be in `crates/ignite/src/apply/plan.rs`, and no unit owns that file for that purpose. This is a risk to the contract and is not a shape finding.
- Out of my lane, acceptance and scope: story:apply-recovery-way-out is the old two-line body. Its scope is `inferred` and broad (`crates/ignite/src/apply`), and it does not name U5, its rows 25 and 26, or its first failing test, unlike the other eight stories.
- Out of my lane, parallel safety: story:store-hosted and story:typed-probe-outcomes collide on `crates/ignite/tests/plan.rs`, and `aep plan artifact waves` reports it. The contract states the merge order for that file.
- Out of my lane, scope: 22.15 says `ignite-perf programs` times `health` "after only". No unit owns a change to that mode if it cannot build the second executable of the todo today.

```findings
- file: .engineering/planning/story/slice-18-fault-matrix.md
  line: 11
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its acceptance rows 20 to 22 read journalRepaired, journalRepairs and the journal.intact report that story:host-journal-integrity produces, and no depends_on edge runs to that story; the only path is via story:apply-recovery-way-out, whose rows 25 and 26 it never reads, so adding depends_on story:host-journal-integrity names the real producer"
```
