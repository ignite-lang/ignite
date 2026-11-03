---
format: aep.planning-md/3
id: review-result:slice-15-scope-round-2
kind: review-result
status: active
title: Scope critic, slice 15 round 2
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

- story:slice-15-frontend — the summary says "the six clauses, E091 and E092", but the body's Out of Scope hands E092 to `story:slice-15-claim-model`. The summary should drop E092, as the Scope and Acceptance already do, so that one item claims E092 — `.engineering/planning/story/slice-15-frontend.md:7` (summary), contradicted by `.engineering/planning/story/slice-15-frontend.md:72` and the claim-model Scope ("E090, E092 and E091's second trigger").

**What you read:** 8 artifacts and 1 source doc. I ran `aep plan artifact show` for the epic and all seven stories, and `aep plan artifact graph`. I read `docs/protocols.md` section 3 (3.1 to 3.3, 3.7) and the P2 row of section 7, and `docs/slice.md` 4.3.

I extracted 14 promises from the epic and the P2 row and traced all 14 to exactly one item:
- vocabulary module: frontend
- `rfc import` and `rfc check`: importer
- 854/855/1143 prose catalogues: catalogues
- 9112 sections 2 to 9 and the 9110 server subset: catalogues
- the six clauses: frontend (syntax) and claim-model (semantics)
- literal-constant evaluation: claim-model
- `ignite conformance`: conformance-command
- E090 and E092: claim-model
- E091: frontend (first trigger) and claim-model (second trigger)
- claims in `telnet.ignite` and `http.ignite`: protocol-claims
- the three P2 acceptance tests: protocol-claims
- "every row of the contract's acceptance table holds": protocol-claims
- the contract section: contract
- "importer reads a local RFCXML file", no network: importer, via its `cargo tree` acceptance

Merge, `task gate` on the slice branch and tag `v0.4.0` are wave-close, so I did not count them as gaps.

**Not a finding, but noted:** `story:slice-15-protocol-claims` also writes `docs/performance/15`, the AGENTS.md "Slice 15" section and the PLAN.md row. The epic does not promise these. They follow the slice-14 close and the "each slice re-measures" rule in AGENTS.md, so I did not count them as reach beyond the parent.

**What I could not establish:**
- Whether a record-typed constant with `Option` defaults and `Text + Text` already checks today. `docs/slice.md` 4.3 does not say. That is design and acceptance lane, not scope.
- Whether the "two statuses on one requirement" diagnostic that D3 will name stays inside E090 to E092. It is open until the contract is written.
- Out of my lane: the Telnet acceptance bullet says "The same for `std/protocol/telnet.ignite`" and relies on inheriting "Rust and LLVM" from the previous bullet. That wording is for the acceptance critic.

```findings
- file: .engineering/planning/story/slice-15-frontend.md
  line: 7
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the summary says the story delivers E092 while its Out of Scope (line 72) and the claim-model story give E092 to story:slice-15-claim-model, so two items read as claiming one outcome; drop E092 from the summary"
```
