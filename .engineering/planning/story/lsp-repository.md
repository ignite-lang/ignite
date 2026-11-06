---
format: aep.planning-md/3
id: story:lsp-repository
kind: story
status: active
title: Move ignite-lsp into its own repository after v0.7.0
relations:
- decomposes: epic:ignite-lsp
scope:
- confidence: cited
  path: crates/ignite/tests/lsp.rs
- confidence: cited
  path: docs/lsp
- confidence: cited
  path: examples/lsp
- confidence: cited
  path: tools/ignite-lsp-types
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-06T07:26:14Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-11-06T07:26:14Z", actor: "human:timo", revision: 5}
---
# Move ignite-lsp into its own repository after v0.7.0

Create `github.com/ignite-lang/ignite-lsp` once an ignite release (v0.7.0 or later) carries
`ignite check --json`, `std.protocol.jsonrpc` and `Launcher` with `Invocation.input`, and move
the server there (design:ignite-lsp, decision 2026-11-05: where the server lives). v0.8.0
(tag on 7a49cd3c3, 2026-11-06) carries all three.

## Scope

- Moved with their history (`git filter-repo`): `examples/lsp/server.ignite`,
  `examples/lsp/lsp_types.ignite`, `examples/lsp/roundtrip.ignite`, `tools/ignite-lsp-types/`,
  `docs/lsp/` (the vendored `metaModel.json` and its README) and `crates/ignite/tests/lsp.rs`.
- The new repository builds against the released `ignite` (`cargo install --git
  https://github.com/ignite-lang/ignite --tag v0.8.0 ignite`), not a path dependency; its
  end-to-end test drives the built `ignite-lsp` with the released compiler.
- ignite keeps no LSP-specific code: the files above, their `Taskfile.yml` task, the workspace
  member and the `AGENTS.md` layout rows leave it in the same change set.
- `ignite-helix` `languages.toml` already names the `ignite-lsp` binary (since 9a95ed0).

## Decision 2026-11-06: what moves

The open question of the first draft (generator, metaModel, end-to-end test) is settled by
ignite-devtools under the approved design: everything LSP-specific moves. Reasons: the generator
exists only to produce `lsp_types.ignite` and its drift test compares against that file, so the
two belong in one repository; the end-to-end test needs the server source; ignite then holds no
code whose only user is another repository.

## Done When

- The server builds and its tests pass in the new repository against a released ignite.
- `examples/lsp/`, `tools/ignite-lsp-types/`, `docs/lsp/` and `crates/ignite/tests/lsp.rs` are
  removed from ignite in one merged PR with a green `task gate`.
