# Reproducing the performance budget

The executable entry point is `scripts/performance.py` (Python standard library only).
It exports immutable Git archives to a source root, uses a separate owned build root, and
writes records/logs outside disposable caches. It never modifies the caller's checkout or
Git index. All commands are sequential; coordinate with other build owners on a shared host.

## Frozen sources

- `origin/bench/11b-recovered` is `df7ccb9bed4440824a2d108fa112eba3fea25f99`, tree
  `5d0bc8b3edc716e4b7473ed860204d548bab2db8`, with parent `60aef38`.
- This is a recovery-time commit of the retained pre-slice-12 snapshot, not an old historical
  implementation commit. All 741 source files match the earlier measurement manifest.
- `baselines/11b-source.json` is that original SHA256 manifest, retained byte for byte;
  `baselines/11b.json` records the Git identity and manifest checksum.
- `9273349` is the published 12b checkpoint. The candidate must be a committed, frozen source
  revision; uncommitted edits cannot enter a Git archive. Commit a checkpoint before measuring.

A fresh clone can fetch the benchmark reference with:

```bash
git fetch origin refs/heads/bench/11b-recovered:refs/remotes/origin/bench/11b-recovered
python3 scripts/performance.py self-check
```

## Running

Use three distinct, non-nested absolute directories outside the checkout. Example:

```bash
python3 scripts/performance.py smoke \
  --baseline origin/bench/11b-recovered --candidate HEAD --comparison 9273349 \
  --sources /home/timo/.cache/scratch/ignite-perf12c/sources \
  --builds /home/timo/.cache/scratch/ignite-perf12c/builds \
  --evidence /home/timo/.cache/scratch/ignite-perf12c/evidence
```

The smoke pass builds each compiler, builds the small todo program, and compares two plans
from the same source revision for deterministic output. Models are not compared across
language revisions because the model schema itself evolved between 11b and 12b.

Once the smoke records are inspected, use the same arguments with `all`. Individual modes
`prepare`, `cold`, `warm`, `programs`, `cli`, and `summarize` support focused work. `prepare`
warms the complete gate and builds the runner runtime. `warm` does that automatically;
`programs` and `cli` require already-built compilers (`prepare` or `cold`). Cold mode cleans
only owned per-ref Cargo targets via `cargo clean`, including artifacts left by smoke.
The source revision and roots must match existing evidence; use new evidence for a new
candidate. Program-build outputs are unique per process, so stale Cargo output is not counted
as a fresh program build.

Defaults implement the performance plan:

- Same toolchain and environment for both sides: incremental off, dev debug line tables,
  release debug off, and `SCCACHE_RECACHE=1` for cold/program builds only.
- Three interleaved warm pairs, fifteen check/plan pairs, and one cold pair. At most two
  extra warm passes and one cold retry replace load-invalid/unchanged-baseline-invalid
  samples. Every required cold component must have a valid pair, not just the aggregate.
- Exclude a pair if either exit fails or start-load delta exceeds 5. Raw records remain.
  Summaries use medians of per-pair ratios and paired absolute medians.
- A candidate failure stays blocking even if later samples pass. Freeze corrected source
  and start a new affected validation batch instead of collecting green runs around it.
- Coverage requires at least 647 passed Rust tests with five ignored and at least 148 inline
  tests on each target. Acceptance also checks every timing ratio, the 10.8 s workspace goal
  under comparable load (default maximum 9), and the 4.6 s `ignite_test.rs` goal.
- All required comparison samples against the published checkpoint must be present, although
  only the frozen 11b source defines the budget. Closure checks its exact Git tree identity;
  alternative baselines are diagnostic and cannot produce an accepted 12c result. Executable counts use actual Cargo
  `Executable …(path)` lines, excluding `target(s)` summary text.

Exit status is 1 if any collected command fails (even when later data is collected), 2 when
an `all`/`summarize` run has incomplete/over-budget evidence, and 0 on command success.
`summary.json` distinguishes candidate failure, baseline failure, missing pairs, coverage and
budget acceptance. Explicitly loosened controls are diagnostic: a different baseline tree,
missing published comparison, fewer requested samples, load limits above 5/9, or retry
limits above two warm passes/one cold pair force `accepted=false` even if timings look green. A successful focused mode alone does not mean the entire budget passes.

The harness records tool versions, environment, exact source commits/hashes, per-command
loads/times/exits/test counts, source integrity, and copied real-clock restart journals.
It refuses unknown build directories, changed source bytes, reused evidence for another
configuration, and storage below the configured guard (20 GiB by default).

## Historical evidence

`12b/` contains the small 12b measurement records, summaries, attribution and copied restart
journals retained when publishing `9273349`. These are historical observations, not 12c
acceptance. Original local paths in the raw records describe where those runs occurred;
use the frozen Git refs and the harness above for reproduction on another machine.
Compiler/program/CLI samples preceded the final test/runtime consolidation where documented
in the report; exact-final gate samples are labelled separately.

The original recovery operation, if auditing another retained copy, is:

```bash
python3 scripts/performance.py recover-baseline \
  --source /path/to/s12-before --manifest /path/to/source-before.json \
  --evidence /path/to/recovery-evidence
```

It verifies every byte and uses a private temporary Git index. An existing benchmark ref
with a different tree is never overwritten. `--publish` explicitly publishes the recovered
benchmark ref; normal measurement modes never create or push Git commits.
