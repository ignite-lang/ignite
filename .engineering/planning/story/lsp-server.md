---
format: aep.planning-md/3
id: story:lsp-server
kind: story
status: draft
title: 'ignite-lsp: an LSP server written in Ignite on the generated message types'
relations:
- decomposes: epic:ignite-lsp
- depends_on: story:lsp-types
- depends_on: story:jsonrpc-protocol
- depends_on: story:check-json-diagnostics
- depends_on: story:slice-17-frontend
revision: 1
---
# ignite-lsp: an LSP server written in Ignite on the generated message types

Step 5 of `design:ignite-lsp`. Message types come only from `story:lsp-types` (generated from
metaModel.json; Timo 2026-11-05: "no, not handwritten"). Diagnostics run through slice 17's
`Launcher` once it is on main.

## Outcome

`examples/lsp/` holds an executable `ignite-lsp`, built by `ignite build`, speaking LSP 3.17 over
stdio on `std.protocol.jsonrpc`:

- lifecycle: `initialize` (full text sync; position encoding utf-8 when offered, else utf-16),
  `initialized`, `shutdown`, `exit` with the spec's exit codes; a request before initialize gets
  -32002;
- text sync: `didOpen`, `didChange` (full), `didClose` keep the open buffers;
- diagnostics: on open and change, `ignite check --json --stdin-for PATH` through `Launcher`,
  published as `textDocument/publishDiagnostics` with code, message, severity and ranges in the
  negotiated encoding.

## Acceptance (inline tests over `Pipe.pair()`, both targets)

1. initialize → initialized → shutdown → exit gives the spec's replies; exit code 0, and 1 when
   exit comes without shutdown.
2. A request before initialize gets -32002; an unknown request -32601; an unknown notification
   is ignored.
3. didOpen/didChange/didClose keep the buffer table right.
4. Byte offset → position is right for 1- to 4-byte characters in utf-8 and utf-16.
5. A buffer with a type error publishes one E040 diagnostic with the right range; fixing it
   publishes an empty list.
6. `conforms [LANGUAGE_SERVER_3_17]` for the rows it implements; `ignite conformance` reports
   them.
7. Helix: `ignite-helix`'s languages.toml names the binary and `hx --health ignite` finds it.

## Out of scope

documentSymbol, hover, definition (need a symbol dump from the compiler).
