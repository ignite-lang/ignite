---
format: aep.planning-md/3
id: review-result:slice-18-parallel-safety-round-1
kind: review-result
status: active
title: Parallel-safety critic, slice 18 round 1
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

- story:store-hosted — it and story:typed-probe-outcomes both land on `crates/ignite/tests/plan.rs` (cited, both scopes), with no `depends_on` edge and no stated order. 22.13 leaves the order open ("merge order U1 then U2, or a trivial append conflict"), and `aep plan artifact waves` puts store-hosted at wave 11 and typed-probe-outcomes at wave 12, the reverse of 22.13. Remedies: a `depends_on` from store-hosted to typed-probe-outcomes naming `plan.rs` as the reason, or have U2's test live in a file U1 does not edit. — .engineering/planning/story/store-hosted.md:14, .engineering/planning/story/typed-probe-outcomes.md:28, `git show origin/feat/slice18-one-service:docs/slice.md` line 7995
- story:typed-probe-outcomes — its stored scope omits files that 22.13 line 7979 gives to U1 (cited): `docs/slice.md` (S2, S3), `docs/layering.md`, `site/book/src/…` and `site/index.html` (S5), `std/protocol/telnet.ignite`, `crates/ignite/tests/apply_transition.rs`, `fixtures/control_trace.ignite`, and the fixtures for E109, E126, E150 and E151. Two collisions follow that the store cannot see. First, `docs/slice.md` is also edited by story:apply-recovery-way-out (17.5 line), with no edge between them. Second, `site/` is also owned by story:slice-18-records; that pair is ordered transitively, so it only needs listing. Remedy: add the missing paths, then either order U1 and U5 on `docs/slice.md` or accept the section-disjoint edit as a stated, deliberate collision. — .engineering/planning/story/typed-probe-outcomes.md:10, `git show origin/feat/slice18-one-service:docs/slice.md` lines 7944 and 7979
- story:apply-recovery-way-out — its scope is the whole directory `crates/ignite/src/apply` plus `crates/ignite/tests/apply_recovery.rs`, both inferred (the only item of the nine on an inferred surface). 22.13 line 7983 owns only `apply/mod.rs`, `apply/host/{transport,admission,controller,recovery}.rs`, `src/main.rs`, a new `tests/apply_way_out.rs` and the `main.rs` mod line. The directory swallows U3's files (ordered by edge) and U1's `apply/plan.rs` and `apply/revision.rs` (no edge). `apply_recovery.rs` is a file the contract does not give U5, and `aep plan artifact waves` reports collisions with slice-13-durability-recovery and slice-13-recovery-gaps on that scope. U5's scope never lists `tests/main.rs`, though U3 and U6 do. Remedy: replace the inferred scope with the 22.13 file list and add `tests/main.rs`. — .engineering/planning/story/apply-recovery-way-out.md:13
- story:apply-recovery-way-out — its acceptance (row 25) drives the terminal design, whose declaration, goldens, journal version and `REDUCER_EVALUATOR` story:typed-probe-outcomes rewrites. Its only edge is to story:host-journal-integrity, so the store's waves put it (11) before typed-probe-outcomes (12) while 22.13 orders U1 in W2 before U5 in W3. Remedy: a `depends_on` from apply-recovery-way-out to typed-probe-outcomes, or a stated reason why the e2e is independent of the design's contract shape. This rests on an inferred dependency (the design file as fixture), not a traced read. — .engineering/planning/story/apply-recovery-way-out.md:10 (relations), `git show origin/feat/slice18-one-service:docs/slice.md` lines 7490 and 7909

**What you read:** the nine stories and the epic via `aep plan artifact show`. I also ran `aep plan artifact waves` (the full collision list), `aep plan artifact list`, a `git grep` over `tools/ignite-perf/src`, and 22.0 to 22.17 of `docs/slice.md` at `origin/feat/slice18-one-service` (after `git fetch origin`).

Surface counts:
- **Placed:** 8 items cited, 1 inferred (apply-recovery-way-out), 0 unplaceable.
- **Acceptance reads traced to a producer in the set:** 8. Seven are recorded by a direct `depends_on`:
  - U4 reads U1 and U2.
  - U5 reads U3.
  - U6 reads U4.
  - U7 reads U4.
  - U8 reads U6 and U7.
- **The eighth:** U6's rows 20 to 22 read U3's repair record, anchor and `inspect` report. They are ordered only transitively, U6 to U5 to U3, which I treated as sufficient.
- **Not traced:** I found no read of U1 or U5 that rests on state they create without an edge, except the inferred one in the fourth finding.

**What I could not establish:**
- Whether U2's `implicit_host` change alters any existing golden once slice 17 merges, because `studio.model.json` is not on `main`. On `main`, `todo.model.json` has no assembly, so I expect no change.
- Out of my lane, scope or acceptance: row 27 needs `ignite-perf programs` to time `todod` and `health` separately, but `programs` today times the whole `examples/todo/todo.ignite` build as one unit (`tools/ignite-perf/src/perf.rs:1775`). No unit's outcome owns that change (U7 names only `apply-cycle`), so the producer of row 27's records is missing.

```findings
- file: .engineering/planning/story/store-hosted.md
  line: 14
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "story:store-hosted and story:typed-probe-outcomes both land on crates/ignite/tests/plan.rs (cited, both scopes), no depends_on edge orders them, 22.13 leaves the order open, and aep plan artifact waves puts store-hosted (wave 11) before typed-probe-outcomes (wave 12), the reverse of 22.13; add an edge naming plan.rs as the reason, or split the surface"
- file: .engineering/planning/story/typed-probe-outcomes.md
  line: 10
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the stored scope omits files 22.13 gives to U1 (docs/slice.md S2 and S3, docs/layering.md, site/ S5, std/protocol/telnet.ignite, apply_transition.rs, control_trace fixture, e109, e126, e150 and e151 fixtures), so the collisions on docs/slice.md with story:apply-recovery-way-out (no edge) and on site/ with story:slice-18-records (ordered transitively) are invisible to waves; add the paths and order or accept the first collision"
- file: .engineering/planning/story/apply-recovery-way-out.md
  line: 13
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "scope is the inferred directory crates/ignite/src/apply plus tests/apply_recovery.rs, where 22.13 owns only specific apply/mod.rs, apply/host/{transport,admission,controller,recovery}.rs, src/main.rs and a new tests/apply_way_out.rs; the directory overlaps U1's apply/plan.rs and revision.rs and U3's files, and tests/main.rs (a mod line shared with U3 and U6) is not listed; narrow it to the contract's file list"
- file: .engineering/planning/story/apply-recovery-way-out.md
  line: 10
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its acceptance (22.0 row 25) drives the terminal design that story:typed-probe-outcomes rewrites (declaration, goldens, journal version, REDUCER_EVALUATOR), yet its only edge is to story:host-journal-integrity, so waves places it (11) before typed-probe-outcomes (12) against 22.13's W2 then W3; add a depends_on to typed-probe-outcomes or state why the e2e is independent (inferred dependency)"
```
