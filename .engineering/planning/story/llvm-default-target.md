---
format: aep.planning-md/3
id: story:llvm-default-target
kind: story
status: active
title: LLVM becomes the default target
summary: Commands without --target use the LLVM target; --target rust keeps the Rust target
relations:
- depends_on: story:slice-16-llvm-moves
scope:
- confidence: inferred
  path: Taskfile.yml
- confidence: inferred
  path: crates/ignite/src/lib.rs
- confidence: inferred
  path: crates/ignite/src/main.rs
- confidence: inferred
  path: crates/ignite/tests
- confidence: inferred
  path: site/book
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-05T03:29:03Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-11-05T03:29:03Z", actor: "human:timo", revision: 4}
---
# Story: llvm-default-target

## Outcome

`ignite build`, `ignite test`, `ignite run` and `ignite plan --for-apply` use the LLVM target when
no `--target` is given, so the default build is the fast one; `--target rust` keeps the Rust target.

## Context

- Decision D2 B (`docs/vision.md` section 8, 2026-11-01) made LLVM the default; the code never
  switched (`BuildOptions::default` in `crates/ignite/src/lib.rs`, `target: Target::Rust`;
  contradiction X25). Resolved 2026-11-04: build time is why Ignite exists (`docs/vision.md`
  section 1).
- Measured: `todod` dev 0.286 s LLVM against 3.125 s Rust (`AGENTS.md`, slice 14 table).
- Waits for `story:slice-16-llvm-moves` (U4), which edits the LLVM lowering and removes the
  quadratic compile of `moves_scale.ignite` on LLVM (U1 measured 12.30 s against 1.39 s on Rust
  for 50,000 steps through a call).

## Acceptance

- `ignite build examples/todo/todo.ignite` with no `--target` writes an LLVM-built `todod`;
  `ignite --help` and the book name LLVM as the default; tests and goldens that assumed the Rust
  default pass with an explicit `--target rust` or the new default; `task gate` green.

## Out of Scope

- Removing the Rust target (it stays as the differential check).
