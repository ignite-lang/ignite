---
format: aep.planning-md/3
id: story:slice-15-rfc-importer
kind: story
status: proposed
title: ignite rfc import and ignite rfc check
summary: RFCXML to catalogue module, bcp14 extraction, digest check, offline
relations:
- decomposes: epic:slice-15-protocols-p2
- depends_on: story:slice-15-contract
- depends_on: story:slice-15-frontend
scope:
- confidence: inferred
  path: crates/ignite/src/lib.rs
- confidence: inferred
  path: crates/ignite/src/main.rs
- confidence: inferred
  path: crates/ignite/src/rfc
- confidence: inferred
  path: crates/ignite/tests/fixtures/rfc
- confidence: inferred
  path: crates/ignite/tests/main.rs
- confidence: inferred
  path: crates/ignite/tests/rfc.rs
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-03T16:02:31Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"review_outcome":3}}}
---

# Story: slice-15-rfc-importer

## Outcome

`ignite rfc import` turns a local RFCXML file into a catalogue module that checks, and
`ignite rfc check` says whether a catalogue's digest still matches its source.

## Context

`docs/protocols.md` 3.1, "Extraction": one requirement per sentence with a `<bcp14>` mark
(RFC 7991 2.9), keywords only in capitals (RFC 8174), ids `R<section>_<ordinal>`, the party guessed
from the sentence and reviewed by a human. The digest is stored in the `Spec` constant. Input,
reader, digest and any section filter are the contract's D4 and D6.

## Scope

- `crates/ignite/src/rfc/` (new): RFCXML reading, sentence and keyword extraction, module text
  writer, digest.
- `crates/ignite/src/lib.rs`: one `pub mod rfc;` line in the module list (lines 15 to 18), nothing
  else; `story:slice-15-conformance-command` adds its own line there later.
- `crates/ignite/src/main.rs`: the `rfc import` and `rfc check` subcommands.
- `crates/ignite/tests/rfc.rs` (new, a `mod` line in `tests/main.rs`) with a small RFCXML fixture
  under `crates/ignite/tests/fixtures/rfc/`.
- A dependency only if the contract's D6 chooses one, with the commit line saying what it replaces.

## Acceptance

- Importing the fixture writes a module byte-identical to its golden, twice in a row.
- `ignite check` of the golden exit 0, no output.
- `ignite rfc check` exit 0 on the golden and its source.
- `ignite rfc check` exit 1 naming the catalogue after one byte of the source changes.
- A lowercase "must" in the fixture yields no requirement.
- A `<bcp14>MUST</bcp14>` sentence yields one requirement with `level = Must`.
- `cargo tree -p ignite -e normal` lists no HTTP client crate (no `reqwest`, `ureq`, `hyper`,
  `curl`).

## Depends on

`story:slice-15-frontend` (the vocabulary the golden is checked against).

## Out of Scope

Writing the real catalogues (`story:slice-15-catalogues`).
