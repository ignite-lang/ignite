---
format: aep.planning-md/3
id: story:static-build-from-source
kind: story
status: proposed
title: A static build from source needs no setup and no network
revision: 2
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T03:32:56Z", actor: "human:timo", revision: 2}
---
# Story: static-build-from-source

## Outcome

A compiler built from source does a `--static` build with no extra setup and no network, as the release binary does.

## Context

- Case study gap G27: from source, the first `--static` build needs `rustup target add x86_64-unknown-linux-musl` and cargo network access (`docs/slice.md` 23).

## Acceptance

- `crates/ignite/build.rs` builds and embeds the musl runtime archive when the musl target is installed, as the release build does; when it is not, `ignite build --static` fails before any work with one line naming `rustup target add x86_64-unknown-linux-musl`, exit 3.
- The Rust target's `--static` build runs with `--offline` when the substrate's crates are vendored or cached; otherwise the error names `cargo fetch`.
- Tests for both paths (the missing-target path with a toolchain stub); `docs/slice.md` 23 updated; `task gate` green.

## Scope

- `crates/ignite/build.rs`, `crates/ignite/src` (build driver), `docs/slice.md`, `site/book/src/guide/install.md`. Inferred.
