---
format: aep.planning-md/3
id: story:slice-15-protocol-claims
kind: story
status: proposed
title: Telnet and Http state their claims; P2 acceptance and records
summary: conforms and statuses in std/protocol, the three P2 acceptance tests on both targets, close records
relations:
- decomposes: epic:slice-15-protocols-p2
- depends_on: story:slice-15-catalogues
- depends_on: story:slice-15-conformance-command
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: PLAN.md
- confidence: inferred
  path: STATUS.md
- confidence: inferred
  path: crates/ignite/tests/conformance.rs
- confidence: inferred
  path: docs/performance/15
- confidence: inferred
  path: std/protocol/http.ignite
- confidence: inferred
  path: std/protocol/telnet.ignite
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-03T16:02:31Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"review_outcome":3}}}
---

# Story: slice-15-protocol-claims

## Outcome

`Telnet` and `Http` in the standard library state their conformance claims, the three acceptance
tests of the protocols.md P2 row hold, and the slice's records are written.

## Context

`docs/protocols.md` section 7, P2 acceptance: "Removing one `satisfies` from a MUST makes `ignite
check` fail with E090. The HTTP matrix lists the §7.1 and §3.2 gaps and claims nothing. The Telnet
matrix claims RFC 854 for an NVT that refuses every option." The slice-14 tests over pipes are the
`verifies` evidence.

## Scope

- `std/protocol/telnet.ignite`, `std/protocol/http.ignite`: `conforms`, `scope`, `satisfies`,
  `unsupported`, `not_applicable`, and `verifies` on the existing inline tests.
- `crates/ignite/tests/conformance.rs`: the three acceptance tests (after
  `story:slice-15-conformance-command`, which creates the file).
- Records at the close: `STATUS.md`, `AGENTS.md`, `PLAN.md`, `docs/performance/15/`.

## Acceptance

- A test copies `std/protocol/telnet.ignite`, removes one `satisfies` from a MUST-level
  requirement, and `ignite check` of the copy exits 1 with E090 naming that requirement.
- `ignite conformance std/protocol/http.ignite` run on the Rust and on the LLVM target (as D8
  selects the target) prints the RFC 9112 section 7.1 chunked-coding row and the section 3.2 Host
  row as gap, and a claim line that says no.
- `ignite conformance std/protocol/telnet.ignite` run on the Rust and on the LLVM target prints a
  claim line that says yes for RFC 854.
- In that matrix, every RFC 855 option-negotiation row in scope prints pass and names the
  declaration that answers `WONT` or `DONT` as its satisfying declaration; the ids of these rows are
  listed in section 19's acceptance row for this test.
- Every row of the section-19 acceptance table has a `STATUS.md` row, dated, naming the command it
  was read from, and each says pass.
- `target/debug/ignite test std/protocol/telnet.ignite --target all` prints at least 37 passed per
  target.
- `target/debug/ignite test std/protocol/http.ignite --target all` prints at least 41 passed per
  target.
- `AGENTS.md` has a "Slice 15" section with the merge gate's time, counts and exit, and layout rows
  for `std/rfc/`; `PLAN.md` row 15 says done.
- `task gate` exit 0 on the slice branch.

## Depends on

`story:slice-15-catalogues`, `story:slice-15-conformance-command`.
