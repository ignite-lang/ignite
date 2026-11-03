---
format: aep.planning-md/3
id: story:slice-15-catalogues
kind: story
status: active
title: Catalogues for RFC 854, 855, 1143, 9112 and 9110
summary: Prose catalogues by hand, 9112 sections 2 to 9 and the 9110 server subset imported
relations:
- decomposes: epic:slice-15-protocols-p2
- depends_on: story:slice-15-frontend
- depends_on: story:slice-15-rfc-importer
scope:
- confidence: inferred
  path: crates/ignite-check/tests/layering.rs
- confidence: inferred
  path: docs/layering.md
- confidence: inferred
  path: std/rfc/rfc1143.ignite
- confidence: inferred
  path: std/rfc/rfc854.ignite
- confidence: inferred
  path: std/rfc/rfc855.ignite
- confidence: inferred
  path: std/rfc/rfc9110.ignite
- confidence: inferred
  path: std/rfc/rfc9112.ignite
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-03T16:02:31Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"review_outcome":2}}}
- {from: "proposed", to: "active", at: "2026-11-03T16:07:59Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"review_outcome":2}}, correlation: "slice-15-wave-1"}
---

# Story: slice-15-catalogues

## Outcome

The standard library carries the catalogues the two protocols bind to: RFC 854, 855 and 1143 by
hand with `source = Prose`, RFC 9112 and the RFC 9110 server subset imported to the extent the
contract's D4 sets.

## Context

`docs/protocols.md` 3.1, "Prose RFCs": 854, 855 and 1143 predate BCP 14 and RFCXML, so their
catalogues quote the text, e.g. "only the IAC need be doubled to be sent as data" [P1]. 9112 and
9110 come from `ignite rfc import` over RFCXML sources at the path the contract's D5 names.

## Scope

- `std/rfc/rfc854.ignite`, `rfc855.ignite`, `rfc1143.ignite`, `rfc9112.ignite`, `rfc9110.ignite`
  (new), and their layering rows (`docs/layering.md`, `crates/ignite-check/tests/layering.rs`) as
  D7 says.
- The RFCXML sources at D5's path, if D5 vendors them.
- The party of each imported requirement reviewed by hand, as protocols.md 3.1 says.

## Acceptance

- `ignite check` of each of the five catalogues exit 0, no output.
- `rfc854.ignite` holds a `source = Prose` requirement whose `text` quotes "only the IAC need be
  doubled to be sent as data"; `rfc855.ignite` and `rfc1143.ignite` each hold at least one
  `source = Prose` requirement quoting its RFC verbatim, listed in the module's `Spec`.
- `ignite rfc check` of `rfc9112.ignite` and `rfc9110.ignite` against the sources at D5's path
  exit 0.
- `rfc9112.ignite` holds RFC 9112 section 7.1's "A recipient MUST be able to parse and decode the
  chunked transfer coding" and section 3.2's Host requirement at MUST level.
- `rfc9110.ignite` holds requirements from exactly the sections D4 lists, and none from others.
- `rfc9112.ignite` holds the extent D4 sets: if truncated, requirements from sections 2 to 9 only,
  at least one from each; if whole, every section's requirements, sections 2 to 9 among them.
- Re-running `ignite rfc import` over each source and diffing against the committed catalogue
  changes only `party =` lines; the commit adding the catalogue states how many it changed.

## Depends on

`story:slice-15-frontend` (the vocabulary checks), `story:slice-15-rfc-importer`.
