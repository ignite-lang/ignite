---
format: aep.planning-md/3
id: review-result:slice-18-parallel-safety-round-2
kind: review-result
status: active
title: Parallel-safety critic, slice 18 round 2
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

I checked all nine items. Every round-1 collision finding is now resolved or stated in a body, and no pair in the set can run at once without the plan saying so.

- **`story:store-hosted` and `story:typed-probe-outcomes` on `crates/ignite/tests/plan.rs`:** `.engineering/planning/story/store-hosted.md:27` now names the shared file and each unit's edit, with merge order U1 then U2. It also says there is deliberately no `depends_on` edge. This is a collision the plan states, which I take as a decision for a person.
- **`story:typed-probe-outcomes` scope:** it now lists every file 22.13 gives U1, including `docs/slice.md`, `docs/layering.md`, `site/book/src`, `site/index.html`, `std/protocol/telnet.ignite`, `apply_transition.rs` and the fixtures and goldens. `aep plan artifact waves` shows each in-set collision as ordered by an edge. Only the `plan.rs` pair has none.
- **`story:apply-recovery-way-out` scope:** the inferred directory is replaced by the 22.13 file list plus `tests/main.rs` and `docs/slice.md`, all cited. It has `depends_on` edges to `story:host-journal-integrity` and `story:typed-probe-outcomes`. Its context names the section-disjoint `docs/slice.md` edit shared with U1.
- **`story:slice-18-fault-matrix`:** it now has a direct edge to `story:host-journal-integrity`, closing round 1's transitive-only read.
- **Row 27's producer:** `story:apply-cycle-timing` now owns separate `todod` and `health` units in `programs`. This was out of my lane in round 1.

**What I read:** the nine stories, `review-result:slice-18-parallel-safety-round-1`, and `docs/slice.md` 22.12 to 22.13 on `main` (lines 7954 to 8014). I ran `git fetch origin`, `git pull --ff-only`, `aep plan artifact graph` and `aep plan artifact waves`. I also ran `git grep` for `implicit_host` and the `e15` filter, and listed the fixtures.

**Surfaces:** 9 cited, 0 inferred, 0 unplaceable.

**Acceptance reads traced to a producer in the set:** 10, and a direct `depends_on` records all 10.
- U4 reads U1 and U2.
- U5 reads U3 and U1.
- U6 reads U3, U4 and U5.
- U7 reads U4.
- U8 reads U6 and U7.

U7's `tools/ignite-perf/src` includes `readiness.rs`, which U1 edits. The pair is ordered transitively through U4, so it needs no finding.

**What I could not establish, and unease that is not a finding:**
- `story:typed-probe-outcomes` does not mention the `plan.rs` sharing. Only `story:store-hosted` does, and it says U1 merges first. `aep plan artifact waves` still puts store-hosted in wave 11 and typed-probe-outcomes in wave 12, so a wave-ordered run lands U2 first. 22.13 accepts "a trivial append conflict" in either order, so I did not count it. A one-line mirror in U1's body would still help its implementor.
- `story:store-hosted`'s fixture `store_hosted.ignite` is not in its scope. It is a new file in `crates/ignite/tests/fixtures`, a directory U1's scope covers whole. The file does not exist yet, so there is no collision.
- Whether U2's `implicit_host` change alters any golden once slice 17's `studio.model.json` is on `main`. Round 1 could not establish this either, and I did not re-check it.

```findings
[]
```
