---
format: aep.planning-md/3
id: story:toolchain-image
kind: story
status: active
title: Publish a digest-pinned toolchain image per release
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: Taskfile.yml
- confidence: inferred
  path: dist/toolchain/Dockerfile
- confidence: inferred
  path: site/book/src/guide/install.md
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-07T23:43:09Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-07T23:43:09Z", actor: "human:timo", revision: 3}
---
# Story: toolchain-image

## Outcome

Each Ignite release publishes one public container image, `ghcr.io/ignite-lang/ignite-toolchain:<version>`, holding everything `ignite build` needs for both targets (the released `ignite` binary and its std, clang and the linker for the LLVM target, the Rust toolchain and the source of the substrate for `--target rust`), pullable without credentials and referenced by digest.

## Context

- Request from the `pusher` session, 2026-11-08: `pusher/pusher-ignite` (spec 0.23.0) sets `toolchain.image` in `pusher-candidate.yaml`; the benchmark runs deps and build inside it. `rust:1.98-trixie` has no clang; the pusher-ignite Dockerfile adds clang in a build stage that is never published. Timo: "planned anyway and quick to build" (relayed by the pusher session).
- Accepted under `~/babelforce/CLAUDE.md` "Upstream story acceptance": it unblocks our work, fakes nothing, costs nothing (public GHCR).

## Acceptance

- `docker run --rm <image>@sha256:<digest> ignite --version` prints the release version, from a host with no registry login.
- Inside the image, `ignite build examples/todo/todo.ignite` succeeds on `--target llvm` and `--target rust`, and the built `todod` answers one HTTP request (a test script run by `task image-check`).
- `task image VERSION=X.Y.Z` builds the image from the release archive and the tag's source; `AGENTS.md` § Releases names the image push and the digest in the release notes.
- The first image is published for the current release and its digest is sent to the `pusher` session.

## Scope

- `dist/toolchain/Dockerfile` (new), `Taskfile.yml`, `AGENTS.md`, `site/book/src/guide/install.md`. Inferred.
