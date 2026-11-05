---
format: aep.planning-md/3
id: story:lsp-types
kind: story
status: active
title: LSP message types generated from metaModel.json
relations:
- decomposes: epic:ignite-lsp
revision: 3
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
