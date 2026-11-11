---
format: aep.planning-md/3
id: story:release-archive-carries-the-substrate
kind: story
status: draft
title: The release archive builds and tests on the Rust target without a clone
revision: 1
---
# Story: the release archive builds and tests on the Rust target without a clone

## Outcome

A driver unpacked from the release archive runs `ignite build --target rust` and `ignite test --target rust` with nothing else installed but Rust: it finds the substrate beside itself. No path of the machine that built the release is in the driver.

## Why

Reported on 2026-11-12 by the first user of v0.16.0, who moved a CI onto the release archive:

- `crates/ignite/src/lib.rs:319` (`default_substrate_path`) is `env!("CARGO_MANIFEST_DIR")/../ignite-io`: the released driver looks for the substrate at the path of the tree that built it. The standard library is found beside the driver (`installed_std_path`, `<prefix>/share/ignite/std`); the substrate is not, and the archive does not hold it.
- `ignite build` has `--substrate`; `ignite test` does not.
- `dist/toolchain/Dockerfile` reads the build path out of `ignite emit` and links it to `/usr/local/src/ignite`, so the image holds a directory named after the build machine's tree.
- Reproduce: in `ghcr.io/ignite-lang/ignite-toolchain:0.16.0` hide `/usr/local/src/ignite` and run the archive's `ignite build --target rust` on any package.

## Acceptance

- `task dist` packs what the Rust target needs to build `ignite-io` outside the repository (the crate, the crates it names by path, and the workspace manifest and lock it inherits from, or a manifest written for the archive) under `<prefix>/share/ignite/`; a driver at `<prefix>/bin/ignite` uses it first, as it does for std. A test unpacks the archive into an empty directory, with the repository not reachable under its build path, and builds and tests a package on the Rust target (the crates may come from the network or a vendored cache; the test states which).
- A driver built by `task dist` holds no absolute path of the build tree: a test searches the binary for the tree's path.
- `ignite test` takes `--substrate`, with the meaning it has for `ignite build`; the book's install page and `ignite help` say where the substrate is looked for, in order.
- The toolchain image no longer creates the build path; `task image-check` passes.
- `AGENTS.md` § Releases is updated; the release note sentence "The Rust target needs a clone (`--substrate`)" goes.
- `task gate` green.

## Scope

- `crates/ignite/src/lib.rs` (`default_substrate_path`), `crates/ignite/src/main.rs` (`test`), `crates/ignite/src/test_build.rs`, `crates/ignite/build.rs`, `Taskfile.yml` (`dist`), `dist/toolchain/Dockerfile`, `site/book/src/guide/install.md`, `AGENTS.md`. Inferred.
