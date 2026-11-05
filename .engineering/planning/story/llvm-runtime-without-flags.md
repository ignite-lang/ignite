---
format: aep.planning-md/3
id: story:llvm-runtime-without-flags
kind: story
status: draft
title: Installed ignite finds its LLVM runtime
relations:
- derived_from: story:llvm-default-target
revision: 1
---
# Story: llvm-runtime-without-flags

## Outcome

An `ignite` installed with `cargo install` builds and tests with the default LLVM target without
`--runtime` and without copying `libignite_rt.a` by hand.

## Context

- Since PR 32 (2026-11-05) LLVM is the default target. `ignite` looks for `libignite_rt.a` next to
  its binary, then in that directory's parent; otherwise it exits 3 and names
  `cargo build -p ignite-rt --features runner` and `--runtime`.
- An installed `ignite` has no runtime archive beside it, so the default build fails until the
  user builds and passes the archive (`site/book` install guide).
- Options: build and install the archive with the CLI, embed it in the `ignite` binary and write
  it to a cache directory on first use, or look it up in the source tree `ignite` was built from.
- `site/book/src/guide/install.md` still shows `ignite 0.3.0` as the version output.

## Acceptance

- A test installs `ignite` into a fresh directory and `ignite build examples/todo/todo.ignite`
  with no flags writes an LLVM-built `todod`; the install guide shows no `--runtime` step.

## Out of Scope

- Cross-compilation.
