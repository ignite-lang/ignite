---
format: aep.planning-md/3
id: story:musl-target-added-later-gives-the-static-runtime
kind: story
status: draft
title: Adding the musl target later gives ignite its static runtime
summary: build.rs emits no rerun trigger when the musl kit is absent, so a target added later stays unseen until a clean build
revision: 1
---
# Story: adding the musl target later gives ignite its static runtime

## Outcome

After `rustup target add x86_64-unknown-linux-musl`, the next `cargo build -p ignite` embeds the static runtime kit, without a clean build.

## Why

`crates/ignite/build.rs` looks for the musl target's `self-contained` directory in the toolchain (`musl_self_contained`, build.rs:95). When it finds none it writes `MUSL_KIT = None` and emits no `rerun-if-changed` for that directory, so cargo keeps the build script's result. A user who adds the target later stays without a static runtime until a clean build: `ignite build --static` and the `llvm_static` tests report "this ignite carries no static runtime: it was built without the Rust target". On 2026-11-24 the wave/17 gate on the build node failed in 44 tests this way: the build script ran at 05:01 and 05:06 CET while the node's toolchain lacked the target, which was complete at 05:08; the result was kept until its directories were deleted by hand.

## Acceptance

- With the target absent, `cargo build -p ignite`, then `rustup target add x86_64-unknown-linux-musl`, then `cargo build -p ignite` again: the second build reruns the build script and the binary carries the kit (a test builds the archive kit check both ways, or the build script emits `rerun-if-changed` on the toolchain's `lib/rustlib/` target directory whether or not the kit is found).
- The same for removing the target: the next build drops the kit and says so.

## Scope

- `crates/ignite/build.rs`. Inferred.
