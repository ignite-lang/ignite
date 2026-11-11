---
format: aep.planning-md/3
id: story:build-time-back
kind: story
status: implemented
title: A dev build, a check and a plan take no longer than at v0.15.0
scope:
- confidence: cited
  path: Cargo.toml
- confidence: cited
  path: crates/ignite-rustgen/src/lib.rs
- confidence: cited
  path: crates/ignite/src/lib.rs
- confidence: cited
  path: crates/ignite/src/llvm_build.rs
- confidence: cited
  path: docs/llvm.md
- confidence: cited
  path: docs/slice.md
- confidence: inferred
  path: quality/baseline.json
- confidence: inferred
  path: tools/ignite-perf/src/remote-user-data.sh
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-11T00:24:53Z", actor: "human:timo", revision: 4, executor: "agent:claude-coordinator", correlation: "wave-11"}
- {from: "proposed", to: "active", at: "2026-11-11T00:24:53Z", actor: "human:timo", revision: 5, executor: "agent:claude-coordinator", correlation: "wave-11"}
- {from: "active", to: "implemented", at: "2026-11-11T23:22:37Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1,"review_outcome":2}}}
---
# Story: a dev build, a check and a plan take no longer than at v0.15.0

## Outcome

Rows 6.2 `llvm`, 6.2 `rust`, 6.1 `all/check` and 6.1 `all/plan` are at or better than v0.15.0 on
the dedicated host.

## Why

Dedicated-host run `iq-20261110t211023z-863a` (HEAD `4520b8cf`, BASE v0.15.0): 6.2 llvm 1295 ->
1875 ms, 6.2 rust 4128 -> 4957 ms, 6.1 check 200 -> 207 ms, plan 201 -> 207 ms.

Diagnosis, counts and instructions per process, no wall-clock
(`.engineering/waves/2026-11-11-wave-11-diagnosis/build.md`):

- The dedicated host has no cargo configuration, so its dev profile is `debug = 2`; the
  workstation's configuration sets line tables only. The host's numbers are reproduced only with
  `CARGO_PROFILE_DEV_DEBUG=2`.
- 6.2 llvm: the runtime archive is linked twice per build (`todod`, `health`) by GNU ld. Archive
  56.6 -> 130.9 MB; section bytes loaded for `todod` 47.5 -> 119.2 MB, 80.7 % of it debug info;
  debug relocations 556,988 -> 1,915,714. The two linker runs are 90 % of the added instructions
  (6,323 M -> 10,501 M in total). The `tls` feature accounts for 49.9 % of the link input; the
  rest of the growth is debug info of `ignite-io`.
- 6.2 rust: TLS is not compiled. The serial chain grew 23,683 M -> 29,427 M instructions:
  `ignite_io` +3,148 M, the system-call wrapper crate +2,018 M (three more features), six new
  compilations (the coroutine crate and what it pulls in). A fresh build makes 20 index requests
  (6 at v0.15.0).
- 6.1: `module_dependencies` runs twice per std module and scans for more builtin names.

Six patches were tried in a copy; they are a starting point: `01-runtime-debug-profile` (build at
99.2 % of v0.15.0's instructions), `02-dev-link-lld` (with 01 and 04: 70.8 %),
`03-rust-seed-lock`, `04-module-dependencies-once` (check and plan at 98.2 %),
`05-nix-without-socket`, `06-rustgen-deps-without-debug` (rust chain at 107.6 %; with
`incremental = false` and 05: 87.3 %).

## Acceptance

- The dev runtime archive carries no debug info of third-party crates and line tables for
  `ignite-rt` and `ignite-io`; a test builds the todo example on the LLVM target against an
  archive built with `CARGO_PROFILE_DEV_DEBUG=2` unset and set, and fails when the dev binary
  holds a compile unit of a third-party crate. The five overrides this replaces (the uncommitted
  change in the release tree) are not kept beside it.
- The dev link on the LLVM target uses the faster linker when it is installed beside the
  resolved clang, and falls back otherwise; the dedicated host's setup installs it
  (`tools/ignite-perf/src/remote-user-data.sh` or what it calls), so the run measures it.
- The generated Rust project builds its dependencies without debug info in the dev profile and
  starts from a lock file, so a fresh build makes no index request.
- `module_dependencies` runs once per module; `plan.json` of the terminal design is byte-identical
  before and after.
- No `unsafe` site is added (row 3.6): patch 05 as written adds four and is not taken in that
  form.
- A dev backtrace of a failing Ignite program on both targets still names file and line of the
  Ignite source and of `ignite-rt` / `ignite-io` frames; the test that pins this passes or is
  added.
- Row 3.2's dev bars are blessed down to what the change measures.
- The dedicated-host run of the release candidate shows the four rows at or better than BASE.
  The coordinator reads this; the unit records its cost as "not measured locally".

## Scope

- `Cargo.toml` (profiles). Cited.
- `crates/ignite/src/llvm_build.rs` (`link`), `crates/ignite/src/lib.rs` (`seed_lock`, the
  dependency cache). Cited.
- `crates/ignite-rustgen/src/lib.rs` (the generated manifest's profile). Cited.
- `tools/ignite-perf/src/remote-user-data.sh` and the host scripts beside it. Inferred.
- `docs/llvm.md`, `docs/slice.md`. Cited.
