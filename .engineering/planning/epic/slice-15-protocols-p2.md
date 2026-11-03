---
format: aep.planning-md/3
id: epic:slice-15-protocols-p2
kind: epic
status: proposed
title: 'Slice 15, wave B protocols P2: RFC catalogues and the conformance matrix'
summary: std/rfc catalogues, conforms/scope/satisfies/verifies and statuses, E090 to E092, ignite rfc import and check, ignite conformance (docs/protocols.md 3, P2 row)
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-03T16:02:31Z", actor: "human:timo", revision: 3}
---

# Epic: slice-15-protocols-p2

## Outcome

A protocol in `std/protocol/` names the RFCs it conforms to, every MUST-level requirement in its
scope carries a status the checker enforces, and `ignite conformance` prints a matrix whose verdict
rests on passing `verifies` tests. Removing one `satisfies` from a MUST makes `ignite check` fail
with E090.

## Context

- `docs/vision.md` 4.1, row "15+": "B (P2 on), C, D, E, G, H ... in that order". Protocols P2 is
  the next phase of wave B; it needs only P1 (slice 14, `epic:slice-14-protocols-p1`, v0.3.0).
- Design: `docs/protocols.md` section 3 (3.1 catalogues, 3.2 linking, 3.3 the matrix, 3.7 tests)
  and the P2 row of section 7. Diagnostic range E090 to E099 (`docs/vision.md` 7, T4); E096 and
  E099 are taken by slice 14.
- `docs/vision.md` 4.1: each slice starts with a contract section in `docs/slice.md`, reviewed
  against `docs/vision.md` section 6 before agents are dispatched. `story:slice-15-contract` is
  that step; the implementation stories carry the protocols.md P2 row until the contract's work
  split names them, and the contract may re-cut them through `aep plan artifact body`.

## Scope

- `std/rfc/`: the vocabulary module and the catalogues for RFC 854, 855 and 1143 (prose, written by
  hand) and RFC 9112 sections 2 to 9 and the RFC 9110 server subset (imported).
- Frontend: record-typed constants holding literal data, the clauses `conforms`, `scope`,
  `satisfies`, `verifies`, `unsupported`, `not_applicable` as contextual words; E090, E091, E092.
- Evaluator: literal-constant evaluation of catalogues and the claim model.
- CLI: `ignite rfc import`, `ignite rfc check`, `ignite conformance`.
- Claims in `std/protocol/telnet.ignite` and `std/protocol/http.ignite`.

## Out of Scope

- ABNF `grammar`, `matches`, generated parsers, E093 to E095 (protocols P3).
- User `interface` and `impl`, transports, TLS, E098 (P4); `std.check.explore`, Q method (P5).
- Closing any HTTP MUST gap: P2 lists the RFC 9112 section 7.1 and 3.2 gaps, it does not fix them.
- Network access at build or test time: the importer reads a local RFCXML file.

## Acceptance

- The three acceptance tests of the protocols.md P2 row hold, as `story:slice-15-protocol-claims`
  states them: removing one `satisfies` from a MUST makes `ignite check` fail with E090; the HTTP
  matrix, run on the Rust and the LLVM target, lists the section 7.1 and 3.2 gaps and its claim
  line says no; the Telnet matrix, on both targets, claims RFC 854 for an NVT that refuses every
  option.
- Every row of the contract's acceptance table holds; `task gate` exit 0 on the slice branch;
  merged to main; tag `v0.4.0`.
