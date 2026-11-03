---
format: aep.planning-md/3
id: review-result:slice-15-scope-round-1
kind: review-result
status: active
title: Scope critic, slice 15 round 1
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

- story:slice-15-contract — the epic promises "RFC 9112 sections 2 to 9 and the RFC 9110 server subset (imported)", and no item decides what that extent is. The importer story has no section or party filter. The catalogues story only says "imported". The contract's "decides at least" list omits both the catalogue extent (truncated at import, or whole RFC narrowed by `scope`) and which 9110 sections or parties make the "server subset". It also omits whether RFCXML sources are vendored, which `story:slice-15-catalogues` hands to the contract. The contract body should name these decisions, and the importer's body should then carry whatever selection the decision requires. — .engineering/planning/epic/slice-15-protocols-p2.md:35, .engineering/planning/story/slice-15-contract.md:40, .engineering/planning/story/slice-15-catalogues.md:41
- story:slice-15-protocol-claims — the epic promises "Every row of the contract's acceptance table holds", and no story's acceptance claims it. The closing story carries only the three P2 tests, `task test-ignite` and the gate. Its scope lists the contract's rows only as `STATUS.md` records, so the promise is narrowed to the three protocols.md rows. Its acceptance should add that every row of the contract's acceptance table holds. — .engineering/planning/epic/slice-15-protocols-p2.md:55, .engineering/planning/story/slice-15-protocol-claims.md:52-57

**What you read:** 8 artifacts: the epic and its 7 stories, each with `aep plan artifact show`, plus `aep plan artifact graph`. I also read `docs/protocols.md` lines 221–440 and 697–716, and `docs/slice.md` 4.3. I extracted about 17 promises from the epic and traced 15 to a story. The other two are the findings above. Of those two, the 9112/9110 extent is claimed by the catalogues story but no item decides what it is.

**What you could not establish:**
- "merged to main; tag `v0.4.0`" (epic line 55) is claimed by no story. The slice-14 epic and its stories (`aep plan artifact show epic:slice-14-protocols-p1`) show the same split, so I treated it as the wave-close step and did not raise a finding.
- Out of my lane (design): `story:slice-15-frontend` puts E092 in `ignite-check`. "Outside the protocol's specs" needs the `Spec` constants' requirement lists, which is the evaluator work in `story:slice-15-claim-model`. The contract's "where E090, E091, E092 are reported" decision may already settle this.
- Out of my lane (parallel-safety): `crates/ignite/tests/conformance.rs` is in the scope of both `story:slice-15-conformance-command` and `story:slice-15-protocol-claims`.
- Out of my lane (acceptance): the prose catalogues for RFC 854, 855 and 1143 have no acceptance beyond `ignite check` exiting 0.

```findings
- file: .engineering/planning/story/slice-15-contract.md
  line: 40
  category: scope
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the epic promises RFC 9112 sections 2 to 9 and the RFC 9110 server subset imported (epic line 35), and no item decides the catalogue extent, what the 9110 server subset is, or whether RFCXML sources are vendored (catalogues line 41 delegates it to the contract); the contract's 'decides at least' list omits them and the importer has no filter"
- file: .engineering/planning/story/slice-15-protocol-claims.md
  line: 52
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the epic promises 'Every row of the contract's acceptance table holds' (epic line 55) and no story's acceptance claims it; protocol-claims lists the rows only as STATUS.md records and its acceptance covers only the three P2 tests, test-ignite and the gate"
```
