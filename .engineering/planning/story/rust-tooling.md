---
format: aep.planning-md/3
id: story:rust-tooling
kind: story
status: active
title: Port the repository's Python scripts to Rust
scope:
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: docs/performance/README.md
- confidence: cited
  path: scripts
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-02T22:38:16Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-11-02T22:38:24Z", actor: "human:timo", revision: 4}
---
## Outcome

The repository's own tooling is Rust. `task gate` and the performance workflow call no Python.

## Scope

| script | lines | caller | replacement |
|---|---|---|---|
| `scripts/check-format.py` | 126 | `Taskfile.yml` `check` (gate) | a Rust binary or `cargo fmt --all -- --check` if the parallel grouping is not needed |
| `scripts/performance.py` | 649 | `docs/performance/README.md` workflow | a Rust binary (workspace member, not linked into compiled programs) |
| `scripts/verify-llvm-readiness.py` | 372 | slice 12c release evidence | a Rust integration test or binary |

Scripts under `docs/performance/` are recorded evidence of past measurements and are not ported.

## Acceptance

- `git ls-files 'scripts/*.py'` is empty; `Taskfile.yml` has no `python3`.
- `task gate` passes with the same checks as before; `cargo fmt` behaviour unchanged.
- The performance workflow in `docs/performance/README.md` names the Rust replacement and produces
  the same record fields.

## Source

Timo, 2026-11-02: "i prefer scripts being done NOT in python, also not so much in bash, ideally
everything is rust". The rule is in `AGENTS.md` (conventions).
