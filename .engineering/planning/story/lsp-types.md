---
format: aep.planning-md/3
id: story:lsp-types
kind: story
status: active
title: LSP message types generated from metaModel.json
relations:
- decomposes: epic:ignite-lsp
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-05T09:32:55Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-05T09:32:55Z", actor: "human:timo", revision: 3}
---
# LSP message types generated from metaModel.json

`design:ignite-lsp`, section "Decision 2026-11-05: generated types" (Timo: "B"). The server's
message types come from the LSP specification's machine-readable `metaModel.json`, never by hand.

## Outcome

- `docs/lsp/metaModel.json` (LSP 3.17 release tag), vendored unmodified with its MIT licence
  notice and its sha256 in a README.
- A Rust binary in the workspace (clap derive) that reads it and writes one Ignite module with
  every structure, enumeration and type alias reachable from a root list: `initialize`,
  `initialized`, `shutdown`, `exit`, `textDocument/didOpen`, `didChange`, `didClose`,
  `textDocument/publishDiagnostics`, `$/cancelRequest`.
- Mapping: structure → record (`extends`/`mixins` flattened, optional → `Option`), enumeration →
  enum with its wire values, `T | null` → `Option<T>`, other `or` types → an enum decoded by
  shape (each arm tried in a fixed order, documented), base types → Ignite types (`decimal` →
  `Json` `Decimal(Text)`), `LSPAny` → `Json`. A type it cannot map is refused with its name,
  never guessed.
- Each generated type carries a comment naming the metaModel entry it comes from; every
  record has `to_json` and `from_json`.

## Acceptance

1. A `#[test]` regenerates the module and compares it byte for byte with the committed file.
2. The generated module checks (`ignite check`) and its round-trip tests pass on both targets:
   for each root message, a JSON example taken from the specification decodes and re-encodes to
   equal `Json`.
3. A root that pulls in an unmappable type makes the generator exit non-zero naming that type.
4. Changing one field in a copy of metaModel.json changes the output and fails the drift test.

## Out of scope

The server (`story:lsp-server`), the ESS route.

## Adversary findings

Adversary pass over 96b6b4f09 and f34977859, 2026-11-05 (failing tests in 6518f334e). The
committed module equals a fresh run byte for byte (not hand-edited); a real Helix 25.07.1
`initialize` round-trips; `Option<Option<T>>`, the `uinteger` range (0 to 2^31-1, per the spec),
keywords, cycles through structures and the licence/digest held. Findings, fixed in 6a96086f5:

- high: an incremental `didChange` whose range did not decode was read as a full replacement
  (the `{text}` arm accepted it). Union decode now commits to the first arm the value selects (an
  object with every required member of the arm present); that arm's error is the union's.
- medium: same cause; a malformed member of a specific arm was dropped by a looser arm. 20 of 44
  unions changed behaviour.
- low: a self-referencing type alias overflowed the generator's stack; now refused with exit 1.
- design note, open for story:lsp-server: a closed enumeration refuses a value it does not list,
  so one unknown value in a client capability `valueSet` fails `initialize`.

Gate: run 1 failed `run::killing_terminald_restarts_it_in_real_time` (restart 558 ms against
the 500 ms bound, load 21.6; the branch has no diff in run/, ignite-io or ignite-eval; 3 of 3
solo runs passed). Run 2 under nice/ionice passed.
