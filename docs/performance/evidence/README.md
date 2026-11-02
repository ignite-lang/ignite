# Recovered measurement evidence

Slices 1 to 12 were built by sessions that kept their scripts, logs and drafts in session
scratch directories under `~/.cache/scratch/<session id>/`, which are deleted 48 hours after
the session ends. The documents in this repository cited those directories. This directory
holds a copy of what they cited, plus the drafts and reviews that never reached a commit,
recovered on 2026-11-02.

## Layout

| directory | source session | what it holds |
|---|---|---|
| `telos/telos-34d976b7/` | `34d976b7-453b-445c-bc5b-857b9e81d67a`, implementation, slices 1 to 10 | slice 9b and 10 measurement scripts and logs (`s9b-*`, `p4_gate*`, `bench*`, `u2-*`), unit prompts and roles, contract drafts (`slice10_contract_draft.md`, `slice6_section.md`, `slice7_section.md`, `slice9_decisions.md`, `n1_section13.md`), reviews r1 to r5 and their verifications (`review-r*.md`, `verified-*.md`), `compare-ess.md`, `throughput-baseline.md`, `syntax-scratch/measure.py` (the corpus tool of `docs/syntax.md` section 2.1), `ess-scratch/`, `tp/` |
| `telos/telos-f769e68e/` | `f769e68e-6990-4c7f-be9b-561eb71b2bb6`, implementation, slices 11 to 12b | slice 11 P0 and P4 (`p0-logs/`, `p0-scripts/`, `p4-logs/`, `p4-scripts/`), slice 11b (`b11-logs/`, `b11-scripts/`), slice 12 P1 to P4 (`s12-p1-logs/`, `p3-measure/`, `s12-p2-bench-unit1cold/todod.log`, `s12-p4-logs/measure/`, `s12-p4-scripts/`), slice 12b rounds (`s12b-*`), roles and contract drafts (`s11_*.md`, `s12_*.md`, `slice11_contract_draft.md`, `slice12_contract_draft.md`) |
| `telos/telos-steer-1f73fa06/` | `1f73fa06-e945-4828-ae60-02b7cdacb054`, steering and review | audits (`effects-audit/`, `io-audit/`, `llvm-audit/` logs and scripts), probes, `todo-bench/` logs |
| `telos/telos-steer-8b795268/` | `8b795268-92d0-4699-8865-fc084cbd02a9`, steering continuation | `sec16.md` (draft of `docs/slice.md` section 16), `slice12_review.md` |

`telos/manifest.json` lists every file with its original path, size and SHA-256 of the
original bytes. Files over 512 KiB are stored gzip-compressed with a `.gz` suffix; the hash
is of the uncompressed content.

Copied: text files that were not already a blob in this repository, from the session
directory itself and from the log, script, measurement and audit subdirectories. Not copied:
`target*/` and every other build output, generated programs (`*-build/`, `*-out/`,
generated `.ll` and Rust), source-tree snapshots, and `.orig`/`.bak` copies of source files
taken before an edit; their final state is in the history of `main`.

## Source trees cited as baselines

| cited as | ref | verification |
|---|---|---|
| `s12-before`, the slice-12 start tree (`docs/slice.md` 16.1, `docs/performance-plan.md` 0) | branch `bench/11b-recovered`, `afa82aa` | all 741 files byte-identical to the scratch copy (`git hash-object`, 2026-11-02) |
| `s12-p4-after2`, the slice-12 close tree (AGENTS.md slice 12 and 12b tables) | tag `bench-12-close`, `d3a0d14` | built from the scratch copy without `.idea/`, `.claude/`, `target/`; 786 files |
| `9273349`, the published 12b checkpoint (`docs/performance/README.md`, 12c records) | tag `bench-12b-checkpoint`, `9273349` | main's history was rewritten after publication; `28dfc75` on `main` has the same tree |

## Already lost

The Codex measurement directories `~/.cache/scratch/ignite12b-codex/` and
`~/.cache/scratch/ignite-perf12c/` no longer existed on 2026-11-02 20:50. What they held
that the documents rely on had been copied into `docs/performance/12b/` and
`docs/performance/12c/` before; paths in those directories' JSON records still name the old
locations, and the files of the same name are next to the records. The Cargo timing HTML/JSON
named in `docs/performance/12b/report.md` was not copied and is gone.
