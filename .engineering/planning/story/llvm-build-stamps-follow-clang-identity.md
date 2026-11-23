---
format: aep.planning-md/3
id: story:llvm-build-stamps-follow-clang-identity
kind: story
status: draft
title: Replacing clang at the same path rebuilds the LLVM objects and the link
revision: 1
---
# Story: replacing clang at the same path rebuilds the LLVM objects and the link

## Outcome

The compile and link stamps of an LLVM build include the identity of the resolved compiler that the target probe already records, so changing the compiler at the same path re-runs compilation and linking, and an unchanged compiler runs neither.

## Why

IG-13, severity medium, class logic; observed with a released command (the maintainer review of 2026-11-23 against main at a1d1407e, v0.16.4 archive).

Synthetic reproducer: a clang wrapper that logs its calls. The first test build called it once for the target probe, three times to compile and once to link. After the wrapper was changed in place, the same passing test called it only for the probe. A third build with the unchanged replacement made no call.

The probe key includes `file_identity(resolve_program(clang))` (`crates/ignite/src/llvm_build.rs:164` to `:168`). The compile stamp (`:651`) and the link stamp (`:680`) are made by `stamp` (`:494` to `:503`) from the compiler path, the arguments and the inputs only.

## Acceptance

- A test with a counting compiler wrapper, red first: replacing the wrapper at the same path re-runs the three compiles and the link; a further build with it unchanged makes no compile or link call.
- Control: an unchanged build after a clean one makes no compile or link call, as today; `task gate` green.

## Scope

- `crates/ignite/src/llvm_build.rs:164` (target probe), `:651` (compile stamp), `:680` (link stamp), `:494` (`stamp`). Each read at a1d1407e, as the review states.
