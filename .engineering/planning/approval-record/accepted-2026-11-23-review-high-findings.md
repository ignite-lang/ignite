---
format: aep.planning-md/3
id: approval-record:accepted-2026-11-23-review-high-findings
kind: approval-record
status: draft
title: 'Ten drafts accepted: the High findings of the maintainer review of v0.16.4'
relations:
- decides: story:executable-names-stay-inside-the-build-directory
- decides: story:named-arguments-carry-parameter-effects
- decides: story:combined-test-runs-keep-every-sibling-suite
- decides: story:run-cache-keyed-by-entry-file
- decides: story:place-receiver-read-before-index-and-prefix-argument
- decides: story:temporary-receiver-evaluated-before-arguments
- decides: story:llvm-short-circuit-with-returning-operand-verifies
- decides: story:executable-names-unique-across-systems
- decides: story:evaluated-bindings-key-of-another-system-is-e108
- decides: story:conditional-property-keeps-its-uses-edge
revision: 1
---
# Ten drafts accepted on 2026-11-23: the High findings of the maintainer review of v0.16.4

Decided on 2026-11-23 by the operator, typed in the coordinating session in answer to one question with three options. The answer was "B".

The question: which findings of the maintainer review of 2026-11-23 (against main at `a1d1407e`, 36 findings) are fixed after they are drafted. Option A read: the two security findings, IG-01 and IG-02, then a patch release. Option B read: all ten High findings, IG-01 to IG-10, in waves, then a patch release. Option C read: drafts only.

What it accepts, in the review's order:

- IG-01 `story:executable-names-stay-inside-the-build-directory`
- IG-02 `story:named-arguments-carry-parameter-effects`
- IG-03 `story:combined-test-runs-keep-every-sibling-suite`
- IG-04 `story:run-cache-keyed-by-entry-file`
- IG-05 `story:place-receiver-read-before-index-and-prefix-argument`
- IG-06 `story:temporary-receiver-evaluated-before-arguments`
- IG-07 `story:llvm-short-circuit-with-returning-operand-verifies`
- IG-08 `story:executable-names-unique-across-systems`
- IG-09 `story:evaluated-bindings-key-of-another-system-is-e108`
- IG-10 `story:conditional-property-keeps-its-uses-edge`

The release that follows the last of them is a patch release, v0.16.5, cut by the steps of `AGENTS.md` § Releases.

Not decided here: the other 26 drafts of the same review (IG-11 to IG-36), and every item that was waiting before this decision.
