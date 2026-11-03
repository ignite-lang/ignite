---
format: aep.planning-md/3
id: review-result:slice-15-acceptance-round-1
kind: review-result
status: active
title: Acceptance critic, slice 15 round 1
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

story:slice-15-catalogues — the acceptance for the hand-written RFC 854, 855 and 1143 catalogues is only that `ignite check` passes with no output, so an empty `Spec` passes; it names no requirement or quoted text that must be present (compare the 9112 bullet at line 49) — .engineering/planning/story/slice-15-catalogues.md:47
story:slice-15-catalogues — `ignite rfc check` against the sources cannot give the same answer twice if the RFCXML files are not in the repository, and the scope leaves vendoring to the contract ("whether they are vendored ... is the contract's decision") while the acceptance assumes the sources are present — .engineering/planning/story/slice-15-catalogues.md:41
story:slice-15-catalogues — "the RFC 9110 server subset" has no section list in the acceptance, and the contract's decision list at slice-15-contract.md:40-53 does not name one either, so nobody can say when the 9110 catalogue is complete — .engineering/planning/story/slice-15-catalogues.md:31
story:slice-15-catalogues — the scope item "The party of each imported requirement reviewed by hand" has no acceptance, so a catalogue with every importer-guessed party passes unchanged — .engineering/planning/story/slice-15-catalogues.md:43
story:slice-15-protocol-claims — "The three P2 acceptance tests pass on the Rust and LLVM targets" is unobservable for the first test, because `ignite check` has no target. The Telnet test names no row or claim line to assert ("claims RFC 854"), so the check cannot be repeated by someone else. The epic repeats the wording at epic/slice-15-protocols-p2.md:51 — .engineering/planning/story/slice-15-protocol-claims.md:54
story:slice-15-protocol-claims — the close records (`STATUS.md` rows, `AGENTS.md` slice 15 gate time and layout rows, `PLAN.md` row 15, `docs/performance/15/`) are in scope with no acceptance line, so they can be skipped and the story still passes — .engineering/planning/story/slice-15-protocol-claims.md:49
story:slice-15-protocol-claims — "`task test-ignite` passes; the telnet and http inline test counts are not lower than at v0.3.0 (37 and 41 per target)" joins two outcomes in one bullet and names no command that prints the per-file counts — .engineering/planning/story/slice-15-protocol-claims.md:55
story:slice-15-claim-model — "is `unclassified` in the model" cannot be read from outside, because the story names no API or output that exposes the status; `ignite conformance` is a later story — .engineering/planning/story/slice-15-claim-model.md:49
story:slice-15-claim-model — the acceptance fixture uses a single MUST. It covers neither MUST NOT, REQUIRED, SHALL or SHALL NOT, nor the "exactly one" status rule the context quotes at line 31, and it covers no limit-versus-`minimum` evaluation, which the scope lists at line 40 — .engineering/planning/story/slice-15-claim-model.md:47
story:slice-15-rfc-importer — "No network access: the command reads only the path it is given" names no check (a test run with the network disabled, or an absent HTTP client dependency), so it can only be asserted — .engineering/planning/story/slice-15-rfc-importer.md:53
story:slice-15-frontend — E091 has two triggers (a non-literal constant and a duplicate requirement id, scope line 43), but the acceptance asks for one fixture per code, so a fixture for either trigger alone satisfies it; the third bullet also joins "parses and checks" with "existing tests and goldens are unchanged" — .engineering/planning/story/slice-15-frontend.md:51
story:slice-15-contract — "a section 19 exists with an acceptance table, decisions, a work split and section-8 rows" can be met by any decisions; it does not require the six decisions listed in the scope (constants compile-time or compiled, clause grammar and where E090 to E092 are reported, importer input, XML reader and digest, how test results are obtained and the output format, work split) — .engineering/planning/story/slice-15-contract.md:57

What I read: 8 of 8 ids (the epic and 7 stories), each with `aep plan artifact kinds`, `aep plan artifact lifecycle story` and a full `cat -n` of its file in `.engineering/planning/`. I also read `docs/protocols.md` 3.1 to 3.3, 3.7 and the P2 row, and ran `git grep`/`ls` on `docs/slice.md` and the tree (no `std/rfc/` exists yet, E090 to E092 are unused, tag `v0.3.0` is present).

What I could not establish:
- Whether the RFC 9112 quote at section 7.1 is in 7.1 or elsewhere in the RFC, because no RFCXML is in the tree and the review is offline.
- Whether a skip exists in `ignite test`, which the verdict "inconclusive" depends on. That is the design critic's lane.
- Out of my lane, not counted: the stories fix E091 and E092 in the checker and E090 in the evaluator, while the contract story lists that as an open decision (slice-15-contract.md:46).

```findings
- file: .engineering/planning/story/slice-15-catalogues.md
  line: 47
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the acceptance for the hand-written RFC 854, 855 and 1143 catalogues is only that ignite check passes with no output, so an empty Spec passes; it names no requirement or quoted text that must be present"
- file: .engineering/planning/story/slice-15-catalogues.md
  line: 41
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "ignite rfc check against the sources cannot give the same answer twice if the RFCXML files are not in the repository; vendoring is left to the contract while the acceptance assumes the sources are present"
- file: .engineering/planning/story/slice-15-catalogues.md
  line: 31
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the RFC 9110 server subset has no section list in the acceptance, and the contract's decision list does not name one, so completeness of the 9110 catalogue cannot be judged"
- file: .engineering/planning/story/slice-15-catalogues.md
  line: 43
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the scope item 'The party of each imported requirement reviewed by hand' has no acceptance, so a catalogue with every importer-guessed party passes unchanged"
- file: .engineering/planning/story/slice-15-protocol-claims.md
  line: 54
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "'The three P2 acceptance tests pass on the Rust and LLVM targets' is unobservable for the ignite check test (no target), and the Telnet test names no row or claim line to assert; the epic repeats the wording at epic/slice-15-protocols-p2.md:51"
- file: .engineering/planning/story/slice-15-protocol-claims.md
  line: 49
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the close records (STATUS.md rows, AGENTS.md slice 15 gate time and layout rows, PLAN.md row 15, docs/performance/15/) are in scope with no acceptance line, so they can be skipped and the story still passes"
- file: .engineering/planning/story/slice-15-protocol-claims.md
  line: 55
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the bullet joins 'task test-ignite passes' with 'inline test counts not lower than 37 and 41 per target' and names no command that prints the per-file counts"
- file: .engineering/planning/story/slice-15-claim-model.md
  line: 49
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "'is unclassified in the model' cannot be read from outside: the story names no API or output that exposes the status, and ignite conformance is a later story"
- file: .engineering/planning/story/slice-15-claim-model.md
  line: 47
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the acceptance fixture uses a single MUST; it covers neither the other MUST-level keywords, nor the 'exactly one status' rule quoted at line 31, nor the limit-versus-minimum evaluation listed in the scope at line 40"
- file: .engineering/planning/story/slice-15-rfc-importer.md
  line: 53
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "'No network access: the command reads only the path it is given' names no check, so it can only be asserted"
- file: .engineering/planning/story/slice-15-frontend.md
  line: 51
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "E091 has two triggers (non-literal constant, duplicate requirement id) but the acceptance asks for one fixture per code, so either trigger alone satisfies it; the third bullet also joins 'parses and checks' with 'existing tests and goldens are unchanged'"
- file: .engineering/planning/story/slice-15-contract.md
  line: 57
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "'section 19 exists with an acceptance table, decisions, a work split and section-8 rows' can be met by any decisions; it does not require the six decisions listed in the scope"
```
