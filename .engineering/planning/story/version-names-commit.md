---
format: aep.planning-md/3
id: story:version-names-commit
kind: story
status: implemented
title: ignite --version names the commit of a source build
scope:
- confidence: inferred
  path: .gitattributes
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: crates/ignite/build.rs
- confidence: inferred
  path: crates/ignite/src/apply/plan.rs
- confidence: cited
  path: crates/ignite/src/main.rs
- confidence: inferred
  path: crates/ignite/src/typed_json.rs
- confidence: cited
  path: crates/ignite/tests/cli.rs
- confidence: inferred
  path: crates/ignite/tests/image.rs
- confidence: cited
  path: dist/toolchain/Dockerfile
- confidence: inferred
  path: docs/research/case-study-gaps.md
- confidence: inferred
  path: site/book/src/guide/install.md
- confidence: inferred
  path: site/book/src/reference/cli.md
revision: 16
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T03:32:55Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-08T21:40:02Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-11-12T12:37:09Z", actor: "human:timo", revision: 16, decided_on: {"recorded":{"test_result":1}}, executor: "agent:claude-coordinator", correlation: "audit-2026-11-12"}
---
# Story: version-names-commit

## Outcome

`ignite --version` names the commit a compiler was built from when it is not a release build, so a build from source is told apart from the release.

## Context

- Case study gap G26: a compiler built at a commit after v0.13.0 prints `0.13.0`.

## Acceptance

- A build from a git checkout prints `ignite X.Y.Z+g<12-hex commit>` (with `.dirty` for a modified tree); `task dist` on a release tag prints `ignite X.Y.Z`; a build without git (the source in the toolchain image) prints the version and the commit recorded at archive time, or `+unknown`.
- `apply::plan`'s compiler pin is unchanged in meaning; the typed document and saved plans name the same string where they name the version.
- Test in `crates/ignite/tests/cli.rs`; `task gate` green.

## Scope

- `crates/ignite/build.rs`, `crates/ignite/src` (version), `Taskfile.yml` (dist), `dist/toolchain/Dockerfile`. Inferred.
