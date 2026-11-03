---
format: aep.planning-md/3
id: review-result:slice-15-acceptance-round-2
kind: review-result
status: active
title: Acceptance critic, slice 15 round 2
relations:
- reviews: epic:slice-15-protocols-p2
- reviews: story:slice-15-contract
- reviews: story:slice-15-frontend
- reviews: story:slice-15-rfc-importer
- reviews: story:slice-15-claim-model
- reviews: story:slice-15-catalogues
- reviews: story:slice-15-conformance-command
- reviews: story:slice-15-protocol-claims
- reviews: story:rustgen-type-parameter-shadowing
revision: 1
---
needs-revision

- story:slice-15-frontend — the fixture acceptance lists `conforms`, `scope`, `unsupported`, `not_applicable` and a `test .. verifies` but not `satisfies`, one of the six clauses the Outcome promises (on fns, types and `Limits` fields), so a frontend that cannot parse `satisfies` still passes — .engineering/planning/story/slice-15-frontend.md:65
- story:slice-15-frontend — the Outcome promises "the clauses emitting nothing", but the acceptance only says `ignite test` passes on both targets, which holds just as well if a clause emitted something harmless; no check compares the emitted output with and without the clauses — .engineering/planning/story/slice-15-frontend.md:67
- story:slice-15-claim-model — the Outcome says the compiler knows "which requirements are in scope", and the Scope names scope by parties and sections, but no acceptance line has a requirement outside `scope` or one narrowed out by party or section, so scope narrowing can be wrong while every listed check passes — .engineering/planning/story/slice-15-claim-model.md:47
- story:slice-15-claim-model — the E092 acceptance covers only "A `satisfies` naming a requirement from a spec the protocol does not conform to", while `docs/protocols.md` 3.2 (line 281) also makes a `verifies` or a status naming an outside requirement E092, so two of the three triggers have no fixture — .engineering/planning/story/slice-15-claim-model.md:52
- story:slice-15-catalogues — the acceptance names the sections `rfc9110.ignite` must and must not hold, but none says what extent `rfc9112.ignite` has (the epic scope says sections 2 to 9), only that it holds two named requirements, so a catalogue holding just those two, or all of RFC 9112, passes — .engineering/planning/story/slice-15-catalogues.md:60
- story:slice-15-protocol-claims — "every option-negotiation requirement in scope satisfied by refusing" names no requirement ids and the matrix prints no reason, so it cannot be checked, and "The same for" leaves open whether both targets run — .engineering/planning/story/slice-15-protocol-claims.md:59
- story:slice-15-conformance-command — the acceptance checks rows per verdict and a claim line of "no" after a failing test, but no fixture shows the claim line saying yes, or its counts (protocols.md 3.3, "claim conformance to rfc9112: no (2 gaps, 0 failures, 1 inconclusive)"), and "inconclusive" is not shown turning a MUST claim to no — .engineering/planning/story/slice-15-conformance-command.md:56

What I read: 9 artifacts of 9 (epic, the 7 slice-15 stories, `story:rustgen-type-parameter-shadowing`), with `aep plan artifact show` on each and `aep plan artifact kinds` and `lifecycle story` and `lifecycle epic`. I read `docs/protocols.md` 3.1 to 3.3, 3.7 and the P2 row, and checked these against the tree:
- `lib.rs:1115` is `Checked::check`.
- The `units.rs` literal lines match.
- The ignored golden test carries the story id.
- `STATUS.md` already shows 37 and 41 passed.

What I could not establish:
- Whether `rfc855.ignite` and `rfc1143.ignite` quotes can be verified verbatim, since the RFC text is not in the tree and I did not fetch it.
- `target/debug/ignite` in the tree is stale (E001 on std generics), so I did not re-run the 37 and 41 counts and relied on `STATUS.md` (2026-11-03 07:09Z).
- Dependence of the catalogue acceptance on D5's vendoring choice is left to the contract, as instructed, and is not a finding.

Out of my lane:
- Parallel-safety: `crates/ignite/src/lib.rs`, `crates/ignite/src/main.rs` and `crates/ignite/tests/main.rs` are shared between the importer and conformance stories. The ordering already declared there handles it.
- Scope: protocols.md says the HTTP matrix "claims nothing", and nowhere does a story state a claim line for the RFC 855 and 1143 specs of Telnet.

```findings
- file: .engineering/planning/story/slice-15-frontend.md
  line: 65
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the fixture acceptance lists conforms, scope, unsupported, not_applicable and a test verifies but not satisfies, one of the six clauses the Outcome promises, so a frontend that cannot parse satisfies still passes"
- file: .engineering/planning/story/slice-15-frontend.md
  line: 67
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome promises the clauses emit nothing but the acceptance only says ignite test passes on both targets, which holds if a clause emitted something harmless; no check compares emitted output with and without the clauses"
- file: .engineering/planning/story/slice-15-claim-model.md
  line: 47
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome says which requirements are in scope and the Scope names scope by parties and sections, but no acceptance line has a requirement outside scope or narrowed out by party or section, so scope narrowing can be wrong while every listed check passes"
- file: .engineering/planning/story/slice-15-claim-model.md
  line: 52
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the E092 acceptance covers only a satisfies naming a requirement outside the conformed specs, while protocols.md 3.2 also makes a verifies or a status naming an outside requirement E092, so two of three triggers have no fixture"
- file: .engineering/planning/story/slice-15-catalogues.md
  line: 60
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the acceptance names the sections rfc9110.ignite must and must not hold but says nothing of rfc9112.ignite's extent (the epic scope says sections 2 to 9), only two named requirements, so a catalogue of just those two or of all of RFC 9112 passes"
- file: .engineering/planning/story/slice-15-protocol-claims.md
  line: 59
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "every option-negotiation requirement in scope satisfied by refusing names no requirement ids and the matrix prints no reason, so it cannot be checked, and 'The same for' leaves open whether both targets run"
- file: .engineering/planning/story/slice-15-conformance-command.md
  line: 56
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the acceptance checks rows per verdict and a claim line of no after a failing test, but no fixture shows the claim line saying yes or its counts, and an inconclusive MUST row is not shown turning the claim to no"
```
