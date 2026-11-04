# Build speed of one program

What one `ignite build` costs in wall time, CPU time and memory, on both targets, next to a
handwritten Rust program of similar size. The landing page's "Build speed" section shows the
medians below. Raw samples: [`samples.txt`](samples.txt).

CPU seconds (user plus system, the compiler and every process it starts) are the figure that
matters when many agents build at once: N concurrent builds need about N times the CPU seconds
of one, while the wall time of one build depends on how many cores are free.

## Medians

Five runs per build, median of each column taken separately. Wall and CPU in seconds, peak RSS
in MiB.

| program | target, profile | wall s | CPU s | peak RSS MiB |
|---|---|---:|---:|---:|
| todo, Ignite (146 lines) | LLVM, dev | 0.224 | 0.223 | 80.1 |
| todo, Ignite | LLVM, release | 0.749 | 0.743 | 93.8 |
| todo, Ignite | Rust, dev | 2.460 | 3.271 | 289.5 |
| todo, Ignite | Rust, release | 3.187 | 6.274 | 307.5 |
| ember, Ignite (1,069 lines) | LLVM, dev | 0.193 | 0.188 | 80.6 |
| ember, Ignite | LLVM, release | 0.583 | 0.581 | 92.4 |
| ember, Ignite | Rust, dev | 3.129 | 3.938 | 290.8 |
| ember, Ignite | Rust, release | 3.784 | 6.639 | 311.8 |
| ember, handwritten Rust (1,461 lines) | cargo, dev | 1.011 | 1.761 | 200.3 |
| ember, handwritten Rust | cargo, release | 4.532 | 4.782 | 225.2 |

Ratios read from the table, CPU seconds:

- todo, Rust target over LLVM target: 14.7 (dev), 8.4 (release).
- ember, handwritten Rust over the Ignite LLVM target: 9.4 (dev), 8.2 (release); wall time 5.2
  (dev), 7.8 (release).

Eight `ignite build examples/todo/todo.ignite --target llvm` started at once, one sample each:

| profile | wall s, all eight | CPU s, all eight | CPU s per build |
|---|---:|---:|---:|
| dev | 0.343 | 2.357 | 0.295 |
| release | 1.373 | 8.907 | 1.113 |

The CPU seconds per build rise under concurrency (0.223 to 0.295 dev, 0.743 to 1.113 release);
the cause (shared caches, lower clock with more cores busy) was not measured.

## What was built

- todo: `examples/todo/todo.ignite`, an HTTP JSON service over `Store<Todo>`, one file, one
  executable `todod`.
- ember: a RESP2 key-value server, one file `ember.ignite` (1,069 lines, 25 of its blocks are
  inline tests, which `ignite build` does not compile), one executable `emberd`; and the
  handwritten Rust implementation of the same server from the same comparison (`src/`, 1,461
  lines, dependencies `hashbrown` and `memchr`; its `tests/` are not built by `cargo build`).
  Both live in a local, unpublished examples repository at commit `bcbe5a3`; the Rust copy was
  built from a scratch copy of its `Cargo.toml`, `Cargo.lock` and `src/`.

## Method

- Commit `f2881ab` of this repository (v0.4.0 plus a planning commit), 2026-11-04. The LLVM
  builds ran 02:50 to 02:52 UTC, the cargo builds 03:08 to 03:10 UTC. `main` later merged
  `1da73c6` (a heap-trim change in `ignite-io` and `ignite-rt`, 152 added lines); not
  re-measured.
- Machine: Intel Core i9-10900K, 10 cores, 20 hardware threads (`nproc` 20), 62 GiB RAM, Linux
  6.18.55, rustc and cargo 1.99.1, clang 23.1.1. Shared with other sessions' builds and tests:
  the 1-minute load average at the start of each run is in `samples.txt` (6.0 to 11.0).
- Compiler: `cargo build --release -p ignite -p ignite-rt`, so `target/release/ignite` and
  `target/release/libignite_rt.a`. The LLVM builds link that prebuilt runtime archive with
  `--runtime`; building it once is not part of any figure.
- Commands, each with a fresh `--out` directory (`--std` points at this tree's `std/` for
  ember):
  - `ignite build examples/todo/todo.ignite --target llvm [--release] --out OUT --runtime target/release/libignite_rt.a`
  - `ignite build examples/todo/todo.ignite --target rust [--release] --out OUT --target-dir OUT-cargo`
  - `cargo build [--release] --manifest-path ember-rs/Cargo.toml --target-dir OUT-cargo`
- Every Rust-target and cargo build had a fresh cargo target directory and ran with
  `RUSTC_WRAPPER=` (sccache off), so they are cold builds that also compile the dependencies
  (for the Rust target, the substrate crate `ignite-io` and its dependencies). With sccache on,
  the compile work runs in the sccache server and the client's CPU time would not count it.
  A warm rebuild inside an existing target directory was not measured; the LLVM target keeps
  no cache between builds, so each of its figures is a build from source.
- Environment of every build: `nice -n 10 ionice -c3`. Cargo builds also had
  `CARGO_BUILD_JOBS=8`, `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=line-tables-only` and
  `lld` as the linker (the machine's cargo configuration). The handwritten Rust program's own
  release profile is `opt-level = 3`, `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`;
  that profile is tuned for run time and accounts for much of its release build time.
- Timing: zsh's `time` with `TIMEFMT='%mE %mU %mS %M'` (wall, user and system milliseconds,
  maximum resident set in KiB), because GNU `time` is not installed on the machine. These are
  the `getrusage` figures of the build's process tree. Peak RSS is that of the largest single
  process in the tree (one `rustc` or `clang`), not a sum.
- Median: for each build, the third of five sorted values, per column; CPU is user plus system
  per run, then the median.

## Columns of `samples.txt`

`build run wall_ms user_ms sys_ms max_rss_kib load1`. A build name ending in `-x8` is the
eight-way concurrent run: one sample of the whole batch, its RSS that of the largest process.
