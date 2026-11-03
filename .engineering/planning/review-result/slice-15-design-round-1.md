---
format: aep.planning-md/3
id: review-result:slice-15-design-round-1
kind: review-result
status: active
title: Design critic, slice 15 round 1
relations:
- reviews: epic:slice-15-protocols-p2
- reviews: story:slice-15-contract
- reviews: story:slice-15-frontend
- reviews: story:slice-15-rfc-importer
- reviews: story:slice-15-claim-model
- reviews: story:slice-15-catalogues
- reviews: story:slice-15-conformance-command
- reviews: story:slice-15-protocol-claims
revision: 1
---
needs-revision

- story:slice-15-frontend — it reports E092 and the duplicate-id half of E091 in `ignite-check`, but both need each `Spec`'s requirement set, which the evaluator computes and which `story:slice-15-claim-model` owns. The frontend's own Out of Scope line says the requirements in scope are what the evaluator computes. `ignite-check` depends only on `ignite-syntax`, so it cannot evaluate a constant. Either move the duplicate-id and E092 checks, with fixtures, into claim-model next to E090, or have the frontend body say the checker resolves membership by name from the `Spec` list literal. Today the frontend's E092 fixture reads an evaluation that a later story produces, and no edge says so. — `.engineering/planning/story/slice-15-frontend.md:42-44`, `:51`, `:57`; `.engineering/planning/story/slice-15-claim-model.md:33`; `crates/ignite-check/Cargo.toml:9-11`
- story:slice-15-rfc-importer — it writes module text in the vocabulary that `story:slice-15-frontend` defines, but its acceptance never checks that the golden passes `ignite check`, and it has no edge to the frontend. The first check of importer output against the vocabulary therefore happens in `story:slice-15-catalogues`, two waves later. Add `depends_on story:slice-15-frontend` and make "the golden checks with exit 0" an acceptance line. This costs nothing: the importer moves from wave 2 to wave 3, and the critical path (contract, frontend, claim-model, conformance-command, protocol-claims) stays at five waves. — `.engineering/planning/story/slice-15-rfc-importer.md:48-49`; `aep plan artifact graph` (importer depends only on the contract)
- story:slice-15-frontend — it adds AST clauses on `protocol`, `fn`, `type`, fields and `test`, and `std/rfc/rfc.ignite`. No story scopes `ignite-rustgen` or `ignite-llvm`, and the frontend's acceptance never builds or tests a clause-bearing fixture on either target. `docs/protocols.md` 6.1 says the clauses "emit nothing", but the first thing to exercise both backends on clauses is `story:slice-15-protocol-claims` (wave 5), so a backend failure shows up in the last story. Add to the frontend acceptance that `ignite test` of a fixture with `conforms` and `verifies` passes on both targets. The contract's compile-time-only versus compiled decision lands in this story. — `.engineering/planning/story/slice-15-frontend.md:52-53`; `docs/protocols.md:660`; `.engineering/planning/story/slice-15-protocol-claims.md:54-55`

What I read: 8 artifacts, with `aep plan artifact show` on each, then `relations`, `graph`, `validate` and `waves --status draft`. I read `docs/protocols.md` 3.1 to 3.3, 3.6, 3.7, 6.1 and the section 7 P2 row, `docs/slice.md` 4.3, and `crates/ignite-check/Cargo.toml`. I walked all 44 edges in the graph, including those outside the set (slice 13 and 14 artifacts, `story:rustgen-type-parameter-shadowing`). There is no cycle. The `depends_on` edges inside the set run contract, then frontend and importer, then catalogues and claim-model, then conformance-command, then protocol-claims. I traced 12 acceptance reads to a producer inside the set. Eleven are recorded by an edge, one of them transitively (`story:slice-15-protocol-claims` reading E090 through `story:slice-15-conformance-command` and `story:slice-15-claim-model`). The twelfth, the frontend's E092 reading evaluated `Spec` membership, runs the opposite way and has no edge. `aep plan artifact validate` reports valid.

What I could not establish:
- Whether `ignite-check` can already resolve `Spec` membership by name. I looked only at its crate dependencies, not its symbol tables. The contract may settle this, which is why the first finding offers both fixes.
- Out of my lane: the three file collisions that `waves` reports are already ordered by `depends_on` or by the waves, so they are parallel-safety's. The 5-wave ordering is not a defect. Unease, not a finding: the Telnet half of `story:slice-15-catalogues` and `story:slice-15-protocol-claims` needs only the hand-written catalogues, so it waits on the importer and on the HTTP catalogues for no reason of its own. The catalogues and protocol-claims stories could each be split along that line. Whether `docs/protocols.md` 3.7 says that `ignite conformance` checks `minimum` against every system's override is for the scope critic to place. I did not check whether any story covers it.

```findings
- file: .engineering/planning/story/slice-15-frontend.md
  line: 42
  category: design
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "E092 and the duplicate-id half of E091 are reported in ignite-check but need each Spec's requirement set, which the evaluator (story:slice-15-claim-model) computes and ignite-check, which depends only on ignite-syntax, cannot; move them into claim-model with E090 or state that the checker resolves membership by name from the Spec list literal"
- file: .engineering/planning/story/slice-15-rfc-importer.md
  line: 48
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the importer writes module text in the vocabulary story:slice-15-frontend defines, its acceptance never checks the golden with ignite check and no depends_on edge to the frontend exists, so the first check of importer output happens in story:slice-15-catalogues; add depends_on story:slice-15-frontend (no change to the critical path) and a golden-checks acceptance line"
- file: .engineering/planning/story/slice-15-frontend.md
  line: 52
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "no story scopes ignite-rustgen or ignite-llvm and the frontend acceptance never builds or tests a clause-bearing fixture on either target, so the claim that clauses emit nothing is first exercised by story:slice-15-protocol-claims in wave 5; add a both-target ignite test of a conforms/verifies fixture to the frontend acceptance"
```
