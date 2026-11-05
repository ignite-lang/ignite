---
format: aep.planning-md/3
id: epic:ignite-lsp
kind: epic
status: active
title: 'ignite-lsp: a Language Server written in Ignite, derived from the LSP specification'
summary: Diagnostics for .ignite buffers from an LSP 3.17 server written in Ignite (examples/lsp/server.ignite), on message types generated from the LSP metaModel.json, with the protocol rules as conformance catalogues.
relations:
- depends_on: story:slice-17-frontend
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-05T15:23:30Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-11-05T15:23:30Z", actor: "human:timo", revision: 5}
---
# Epic: ignite-lsp, a language server written in Ignite

Design: design:ignite-lsp.

## Outcome

An editor (Helix first) shows the compiler's diagnostics for an `.ignite` buffer as it is typed,
from a server that is itself an Ignite program: `examples/lsp/server.ignite`, built by
`ignite build examples/lsp/server.ignite --out DIR` to `DIR/bin/ignite-lsp`. It is known to have
happened when `ignite conformance examples/lsp/server.ignite` prints
`lsp-3.17: yes (0 gaps, 0 failures, 0 inconclusive)` and an E040 in an open buffer reaches the
client as a `publishDiagnostics` range (`crates/ignite/tests/lsp.rs`, both targets).

## Why Now

Ignite code was edited with no feedback before `ignite check` ran by hand. Slice 17 brought
`Launcher`/`ChildProcess`, so a server can run the compiler; the grammar and queries in
`ignite-helix` gave highlighting but no diagnostics. The server also exercises the standard
library (JSON, JSON-RPC, process, RFC catalogues) on a real protocol.

## Scope

Built (all merged into main, the last in https://github.com/ignite-lang/ignite/pull/44, 59f0375bf):

| story | delivered |
|---|---|
| story:check-json-diagnostics | `ignite check --json --stdin-for PATH`: diagnostics of every module as JSON |
| story:rfc-spec-id | `std.rfc` `Spec`/`Requirement` gain `id: Text` for non-RFC specifications |
| story:jsonrpc-protocol | `std.protocol.jsonrpc` (framing, envelopes, `serve`, `split_frame`) and the JSON-RPC 2.0 and LSP base catalogues in `std/rfc/` |
| story:json-decimals | `std.json` `Decimal` |
| story:lsp-types | `tools/ignite-lsp-types`: LSP 3.17 types generated from the vendored `docs/lsp/metaModel.json` into `examples/lsp/lsp_types.ignite`, drift-checked |
| story:lsp-server | `examples/lsp/server.ignite`: lifecycle, full text sync, diagnostics via `Launcher` with `Invocation.input`, utf-8/utf-16 positions, `conforms` claim |

Remaining: story:lsp-repository (draft): move the three `examples/lsp/` files with their history
into `github.com/ignite-lang/ignite-lsp` after v0.7.0.

## Out of Scope

- hover, definition, completion, document symbols: need types and spans from the compiler; a later
  epic (design:ignite-lsp §5).
- incremental `didChange`: the server advertises Full sync.
- diagnostics of other files (a sibling module or std) are placed at 0:0 of the open document with
  that file's path in the message.
- the lifecycle as an `ess-protospec/1` machine; the ESS route for the types (design:ignite-lsp,
  decision 2026-11-05: generated types).

## Risks

- Release timing: the repository move waits on a release that carries `check --json`,
  `std.protocol.jsonrpc` and `Invocation.input`; until then the server lives in `examples/lsp/`.
- Large documents: one check runs per change on the session thread; 1,000 diagnostics on a 1.1 MB
  document take 1.23 s (debug build), during which the server answers nothing.

## Ambiguities

- Which files besides the three `examples/lsp/` files move with story:lsp-repository
  (`tools/ignite-lsp-types`, `docs/lsp/metaModel.json`, `crates/ignite/tests/lsp.rs`):
  inferable at that story; recorded in its body.
- Helix wiring (`[language-server.ignite-lsp]` in `ignite-helix` `helix/languages.toml`):
  inferable, design:ignite-lsp §4 step 6; done once a binary is installed.

## Done When

- story:lsp-repository is implemented: `ignite-lsp` builds and passes its tests in its own
  repository against a released ignite, and `examples/lsp/` is removed from ignite.
- `ignite-helix` `helix/languages.toml` names the `ignite-lsp` binary for the `ignite` language.

## Decision 2026-11-05: where the server lives

Operator decision (Timo, 2026-11-05, option A, relayed by the ignite session): `feat/lsp-server`
merges into ignite main with the server in `examples/lsp/` as it is. The `ignite-lsp` repository is
created after v0.7.0, once a release carries `ignite check --json`, `std.protocol.jsonrpc` and
`Launcher` with `Invocation.input`; the move is `story:lsp-repository`. This replaces the
"new repository `ignite-lsp`" row of the layers table until then.
