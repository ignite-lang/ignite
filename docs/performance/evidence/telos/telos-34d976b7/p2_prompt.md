You work in /home/timo/projects/telos, a Rust compiler for the Telos language with two targets:
- Rust: generated crates built by cargo.
- LLVM: IR that clang builds, linked with `libtelos_rt.a`, the C ABI adapter in `crates/telos-rt`.

Both targets use one blocking substrate, `crates/telos-io` (std::net, one OS thread per task). Read `AGENTS.md`, `docs/llvm.md` and `docs/slice.md` sections 6 and 7.

Timo's request: "measure throughput of the server-side, benchmark both, make the telos version faster as the rust one". "The telos version" is the LLVM-target build. Phase 1 is done. Read `/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/throughput-baseline.md` in full: it has the method, tables, profiles and a ranked list of changes. Phase 1 measured the slice-3 tree, where the Rust target still ran on tokio. Since then, slice 4 moved both targets onto one blocking substrate, and slice 6 changed the surface syntax, including the rewritten `std/protocol/telnet.telos`.

You are P, phase 2. Another agent, F, works at the same time on small compiler fixes in `crates/telos-syntax`, `crates/telos-check`, `crates/telos-types` and `crates/telos-rustgen`. Don't edit those crates. You own:
- `crates/telos-io/**`, `crates/telos-rt/**` and `crates/telos-llvm/**`;
- the new `crates/telos-load/**`;
- `std/protocol/telnet.telos`;
- the workspace `Cargo.toml` (members only) and `Taskfile.yml`;
- `docs/llvm.md`, and `docs/slice.md` section 7.

Steps:
1. Port the load generator from `/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/telos-load-src/` into `crates/telos-load` (a workspace member, no new external dependencies). Add `task bench-throughput`, which builds both targets' release `terminald`, runs the matrix (`rr`, `pipe` and `connect` at the phase-1 concurrency levels, median of 3) and prints one compact table per workload.
2. Measure the current tree (slice 6, before your changes) for both targets, release against release. These are baseline B. Baseline A is the tokio Rust numbers from `throughput-baseline.md`.
3. Implement the ranked changes, measuring after each and keeping only what helps:
   - one write per reply in `std/protocol/telnet.telos` (`answer_line`);
   - write coalescing and `TCP_NODELAY` in `telos-io`;
   - remove the global `HANDLES` mutex from the read and write path in `telos-rt`, for example with per-handle `Arc`s, a lock-free table, or by handing out stable pointers;
   - connection thread reuse, smaller stacks, and an accept path that does not block, in `telos-io`;
   - inline the `arena_due` check and the arena bump allocation in the generated IR;
   - elide deep copies of arguments the callee only reads, in `telos-llvm` lowering (borrowed parameters). This needs a sound read-only analysis. Keep value semantics.
   - anything else the profiles show.
4. Changes in `telos-io` speed up both targets. The goal is that the LLVM build beats the Rust build of the same tree.

Acceptance:
- The LLVM release `terminald` beats the Rust release `terminald` of your final tree on each of `rr` 16, `pipe` 16, `pipe` 256 and `connect` 16, by the median of 3 runs. It also beats baseline A on each.
- If a workload cannot be won, report it with the profile evidence and the remaining gap. Do not tune the benchmark to win.
- `task gate` is green. The 28 terminal cases still pass on both targets, and `llvm_memory` stays flat.
- `docs/llvm.md` describes every lowering or runtime change.

Use `CARGO_TARGET_DIR=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/target-h` for direct cargo calls; `task` targets use the repository's `target/`. The rest of the rules are in `/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/rules.md`; read and follow them. Two additions to those rules:
- Benchmarks are sensitive to load. Record runnable tasks before each run, and re-run contended cells.
- Never call the GitLab API, with `curl` or anything else, using `$GITLAB_PERSONAL_TOKEN` or any other personal token of Timo's.

A `terminald` on port 2323 belongs to Timo; leave it alone.

Final report, at most 15 lines:
- the table for the four workloads: baseline A, baseline B (Rust and LLVM), and final (Rust and LLVM);
- what was kept, with its gain;
- what was dropped;
- test counts.
