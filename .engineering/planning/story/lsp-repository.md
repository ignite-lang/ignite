---
format: aep.planning-md/3
id: story:lsp-repository
kind: story
status: draft
title: Move ignite-lsp into its own repository after v0.7.0
relations:
- decomposes: epic:ignite-lsp
revision: 1
---
# Move ignite-lsp into its own repository after v0.7.0

Create `github.com/ignite-lang/ignite-lsp` once an ignite release (v0.7.0 or later) carries
`ignite check --json`, `std.protocol.jsonrpc` and `Launcher` with `Invocation.input`, and move
the server there (design:ignite-lsp, decision 2026-11-05: where the server lives).

## Scope

- The 3 files move with their history (`git filter-repo` or `subtree split`):
  `examples/lsp/server.ignite`, `examples/lsp/lsp_types.ignite`, `examples/lsp/roundtrip.ignite`.
- The new repository builds against the released `ignite`, not a path dependency.
- Open: whether `tools/ignite-lsp-types`, `docs/lsp/metaModel.json` and
  `crates/ignite/tests/lsp.rs` move too or stay in ignite.
- `ignite-helix` `languages.toml` names the installed `ignite-lsp` binary.

## Done When

- The server builds and its tests pass in the new repository against a released ignite.
- `examples/lsp/` is removed from ignite in the same change set.
