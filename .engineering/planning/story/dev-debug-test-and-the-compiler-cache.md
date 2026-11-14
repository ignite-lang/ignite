---
format: aep.planning-md/3
id: story:dev-debug-test-and-the-compiler-cache
kind: story
status: draft
title: The test of the dev build's debug info passes when a crate comes out of the compiler cache
relations:
- informed_by: story:binary-size-back
revision: 1
---
# Story: the test of the dev build's debug info passes when a crate comes out of the compiler cache

## Outcome

`dev_build::dev_backtrace_names_ignite_and_substrate_lines_on_both_targets` gives the same result whether `ignite-io` of its Rust-target build was compiled in the test's own cargo directory or came out of the compiler cache, and a dev executable built from a cached `ignite-io` has the `.dwo` files its debug units name, or the build does not use the cache for the crates it splits.

## Why

The release gate of v0.16.4 (2026-11-13, head `bb4ac4bf`) failed once on this test and on nothing else: 845 passed, 1 failed. The message: the executable `target/tmp/dev-backtrace-cargo/debug/overflow` names `.dwo` files of `ignite_io` under `target/tmp/differential/cargo/debug/deps`, and they are missing.

What was read:

- `target/tmp/differential/cargo/debug/deps` held `libignite_io-8429e0508e3e1e43.rlib` and no `.dwo` file of `ignite_io`; `target/tmp/dev-backtrace-cargo/debug/deps` held the same rlib and none either.
- The directory `target/tmp` of that tree had been deleted before the gate, with the compiler cache (sccache) left as it was.
- With the two cargo directories removed and the test run alone under `SCCACHE_RECACHE=1`, it passed, and `target/tmp/dev-backtrace-cargo/debug/deps` held 16 `.dwo` files of `ignite_io`. The lane `dev_build::` then passed, 6 of 6.

Inferred, not checked further: the cache returns the rlib of a crate built with `split-debuginfo = "unpacked"` without its `.dwo` files, and two cargo directories of one tree share a cache entry of `ignite-io`, so the skeleton units name the directory of the first compile. A tree whose `target/tmp` was never deleted compiles each directory once and keeps the files, which is why the gates before this one passed.

The same holds outside the tests: a user's dev build whose `ignite-io` comes from a compiler cache would have line tables that point at `.dwo` files that are not there. Whether that happens with the released driver was not tried.

A second reading the same day, on a tree whose whole build output had been discarded (the compiler cache warm): the test failed alone at the same assertion (`crates/ignite/tests/dev_build.rs:464`) before its Rust-target build existed, so the crates of the workspace's own `target/debug/deps` (`ignite_rt`, `ignite_io`, which the LLVM executable's runtime archive is made of) had come out of the cache without `.dwo` files as well. After `cargo clean -p ignite-rt -p ignite-io`, `SCCACHE_RECACHE=1 cargo test --workspace --no-run` and the test alone under `SCCACHE_RECACHE=1`, it passed: 16 `.dwo` files of the newest `libignite_rt` archive, 32 of `ignite_io` in `target/debug/deps`, 16 of `ignite_io` in `target/tmp/dev-backtrace-cargo/debug/deps`. `cargo test -p ignite` builds other crate hashes than `cargo test --workspace`, so the build that is told to compile has to be the one the gate runs.

## Acceptance

- A test deletes the cargo directory of a dev build, builds again with the compiler cache warm, and the executable's `.dwo` files exist (red first on a machine with the cache on; the test says so and passes where no cache is configured).
- `task gate` green after `target/tmp` of the tree was deleted, and green in a tree with no build output and a warm compiler cache.

## Scope

- `crates/ignite/tests/dev_build.rs` (`assert_dev_debug`), the driver's cargo invocation for the Rust target and the dev profile of the substrate (`split-debuginfo`). Inferred.
