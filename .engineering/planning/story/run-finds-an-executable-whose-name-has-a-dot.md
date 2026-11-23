---
format: aep.planning-md/3
id: story:run-finds-an-executable-whose-name-has-a-dot
kind: story
status: draft
title: '`ignite run` finds an executable whose name contains a dot'
revision: 1
---
# Story: `ignite run` finds an executable whose name contains a dot

## Outcome

A component with `executable = "daemon.v2"` is built and started by `ignite run` on both targets, from a fresh build and from a cached one.

## Why

IG-15, severity medium, class logic; observed with a released command (the maintainer review of 2026-11-23 against main at a1d1407e, v0.16.4 archive).

The LLVM build wrote an executable named `daemon.v2`, yet `ignite run` exited 3 saying the build produced no executable.

`by_name` in `build_members` compares the file stem of each built path with the configured name (`crates/ignite/src/lib.rs:1689` to `:1691`); the stem of `daemon.v2` is `daemon`. It is used after a fresh build (`:1722`, the error at `:1723` to `:1735`) and for the cached path (`:1700`). The link output is `bin_dir.join(name)` (`crates/ignite/src/llvm_build.rs:661`).

## Acceptance

- A test, red first: `ignite run` of a component named `daemon.v2` starts it on both targets, and again with the build cached.
- Control: a name without a dot still runs; `task gate` green.

## Scope

- `crates/ignite/src/lib.rs:1690` (`by_name`), `:1722`, `crates/ignite/src/llvm_build.rs:661`. Each read at a1d1407e, as the review states.
- `story:executable-names-stay-inside-the-build-directory` restricts the same names and has to allow the dot.
