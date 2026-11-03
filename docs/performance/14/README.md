# Slice 14 gate time (`docs/slice.md` 18.18)

Measured 2026-11-03 08:12 to 08:47 local time, 20 cores shared with other sessions, sccache on,
every command through the session's build-slot wrapper (`CARGO_INCREMENTAL=0`,
`CARGO_PROFILE_DEV_DEBUG=line-tables-only`, `RUST_TEST_THREADS=4`, `CARGO_BUILD_JOBS=8`,
`nice -n 10`, `ionice -c3`). The record is in `AGENTS.md`, "Slice 14".

Trees:

- Before: a `git archive 340f782` copy, target dir `target-before` in the session's scratch
  directory.
- After: the slice-14 worktree at `65d6cd3` (bb69fad plus the two `cli::` fixes the merge gate
  needed). Its cold gate ran in a scratch `target-after`, which was emptied with `cargo clean`
  straight afterwards. Its warm passes used the worktree's own `target/`, which was warm from the
  merge gate. Both choices kept the session under its 15G disk budget.

Method: one cold gate per tree (fresh target, `SCCACHE_RECACHE=1`; the after tree first), the
before tree's full `task gate`, then three interleaved warm passes in which every unit runs once
on before and then once on after. A pair is dropped when the two start loads differ by more than
5 or either exit is not 0. None was dropped. Ratios are after / before, as the median of the pair
ratios.

## Files

| file | content |
|---|---|
| `pairs.md` | every unit: medians of the kept absolute times, median pair ratio, kept/discarded, each pair with its loads and exits; output of `summarize pairs` over the `*.summary` files |
| `counts.txt` | cargo test counts, the `crates/ignite` end-to-end binary's result line and the inline results per target; output of `summarize counts` over the cold, before full-gate and merge-gate logs (the cold log is split at the before tree's first step) |
| `scripts/env.sh` | shared setup: trees, target dirs, `timed` (one `STEP` line per run, with the 1-minute load at start and end), `gate` (fmt, clippy, `cargo test --workspace -- --skip apply`, `task test-ignite`, each timed, then the total), disk guard |
| `scripts/cold.sh` | the cold gate of each tree, and the untimed release runtimes for the LLVM release builds |
| `scripts/fullgate.sh` | the before tree's full `task gate` |
| `scripts/pass.sh` | one warm pass: warm gate after `touch crates/ignite-check/src/lib.rs`; `task test-ignite` with nothing changed; `--no-run` after the touch; 5 pairs each of `ignite check` and `ignite plan` of the design file; `ignite test std/protocol/{telnet,http}.ignite --target all` cold (fresh `--out` and `--target-dir`, `SCCACHE_RECACHE=1`) and warm (same dirs); todod and terminald, Rust and LLVM, dev and release, fresh `--out` and target, `SCCACHE_RECACHE=1` |
| `scripts/summarize.rs`, `scripts/summarize.Cargo.toml` | the Rust summarizer (`main.rs` and `Cargo.toml` of a scratch crate outside the workspace, renamed so that no tool picks them up as a package) |
| `logs/cold.log`, `.summary` | both cold gates and the runtime builds |
| `logs/fullgate-before.log`, `.summary` | the before tree's full `task gate` |
| `logs/merge-gate-after.log` | the after tree's full `task gate`, the slice's merge gate, at `65d6cd3`. `.summary` was written by hand from the log's `LOAD` line (load at start) and `WALL` line (start and end in epoch seconds) |
| `logs/merge-gate-after-attempt1.log` | the first merge gate at bb69fad: `cli::emit_prints_the_program_and_the_std_crate` failed (it expected `pub fn serve(`, slice 14 emits `pub fn serve<S: …>(`) |
| `logs/merge-gate-after-attempt2.log` | the second at the first fix: `cargo fmt --check` failed on that fix |
| `logs/pass-{1,2,3}.log`, `.summary` | the three warm passes |
| `logs/aborted-pass-{1,2}.log`, `.summary` | the first attempt at passes 1 and 2. Pass 1 stopped after the telnet warm step because `cargo clean` refuses the throwaway build dirs (they have no `CACHEDIR.TAG`), and pass 2 was stopped by hand. The scripts now remove those dirs with `rm`, guarded to the builds directory. None of these runs is in `pairs.md` |
