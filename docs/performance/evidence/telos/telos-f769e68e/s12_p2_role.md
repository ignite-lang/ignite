You work in /home/timo/projects/telos, the Ignite compiler: a Rust compiler with two targets (Rust and LLVM), a standard library in Ignite (`std/`) and a substrate in Rust. Read first:
- `AGENTS.md`, including the layout rules and the "Gate time" section;
- `docs/slice.md` sections 6.1 (generated main, the `listening on` line, `--port`), 7 (the substrate), 14.8 (failure reports) and 16 (your contract, reviewed and accepted by the steering session; amendments A1 to A15 applied);
- `docs/llvm.md`;
- `docs/design.md` sections 7.1 and 11.8;
- `crates/ignite-io/src/*.rs`, `crates/ignite-rt/src/`;
- the rules file `/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/s11_rules.md`; follow it.

You are P2: substrate and adapters. You start when the coordinator tells you P1's skeleton has landed.

| agent | owns |
|---|---|
| P1 | `ignite-syntax`, `ignite-check`, `ignite-types`, `ignite-eval/src/{resolve,validate}.rs`, `docs/slice.md` |
| P2 (you) | `crates/ignite-io/**`, `crates/ignite-rt/**`, the lowering of the three 16.13 builtins in `crates/ignite-rustgen` and `crates/ignite-llvm`, `crates/ignite/src/run/{process,probe,clock,journal,control}.rs` |
| P3 | `crates/ignite-eval/src/{model,value,eval,explain}.rs`, `crates/ignite/src/run/{mod,engine,replay}.rs`, `crates/ignite/src/lib.rs`, `main.rs`, `crates/ignite/tests/**` |
| P4 | `std/**`, `examples/**`, `AGENTS.md`, `docs/friction.md`, design notes, acceptance |

P3 owns `crates/ignite/tests/main.rs` and `common/mod.rs`: ask P3 by SendMessage for your `mod` lines and helpers. Agree the adapter interface (the observation and request types between your adapters and P3's engine) with P3 first, by SendMessage, and keep it in `crates/ignite/src/run/control.rs`. The coordinator gives you the ids.

## Order (A12)

Units 1 to 4 first. Unit 5, the drain, is your last unit: start it only after the coordinator tells you acceptance items 1 to 5 and 7 to 10 of 16.0 pass. If the slice runs long, the close reports the rollout as the carried item.

## Units

1. Substrate builtins (16.13), both targets: `Clock.monotonic()`, `Endpoint.of(ref)` with the `--ref path=address:port` arguments and the "no binding for <path>" error; rustgen calls `ignite-io` directly, ignite-llvm through new `ignite-rt` C ABI entries. Inline tests through `ignite test` on both targets. Builds of programs that use neither are byte-identical: every existing Rust and LLVM golden stays unchanged.
2. Process adapter (`run/process.rs`, 16.2, 16.7): spawn with `PR_SET_PDEATHSIG(SIGKILL)` in the run's process group; A3: prctl ties the signal to the forking thread, so every child, probes included, is spawned from one spawner thread that lives for the whole run, and other threads send it requests. Waiter per child giving `Exited` or `Signalled`; the `listening on <address>:<port>` line gives `Ready` with the port (pinned for restarts); `Kill(member, signal)`.
3. Probe adapter (`run/probe.rs`, 16.8): spawn one probe process per attempt through the spawner thread, read its one stdout line, hand it to P3's decoder, kill it at the deadline the engine names; `AttemptFailed` for a missing executable, a non-zero exit or a line that does not decode.
4. Clock, journal and control input (`run/clock.rs`, `run/journal.rs`, `run/control.rs`; 16.4, 16.5, 16.3): real and manual clock; the settle rule with `--settle` (default 2 s; tests pass 1 s, A2); journal open, header, append with `sync_data` before an effect is dispatched, the lock file (E126 when held), torn final line dropped and reported, `--fresh` rename; stdin control lines `status`, `rollout`, `stop`, `advance <quantity>`.
5. Drain (16.13 drain row, 16.12, last, see Order): a signal thread (`sigwait`, SIGTERM blocked in every other thread) sets drain mode, shuts down the read side of every listener and prints `released <address>:<port>` once each is closed (A5); a pending `accept` fails with "draining"; after `K.main` returns, `run` waits for live tasks, then exits 0; a second SIGTERM exits at once. A6: outside `ignite run` the wait for live tasks is unbounded; write that into the `ignite-io` doc comment. Existing tests end servers with `Child::kill` (SIGKILL), so none of them changes. The `AwaitRelease` adapter reads the `released` line; it never connect-polls. Test: an open session survives SIGTERM, a new connect is refused after `released`, on both targets.

## Gate

- `cargo test -p ignite-io -p ignite-rt` is green; `task test-ignite` is green on both targets.
- `cargo build --workspace --all-targets` succeeds.
- clippy `-D warnings` and fmt are clean on your files.
- No test of yours sleeps for a policy or probe duration; the manual clock drives time.
- Invariant 9: `todod` build time on both targets, dev and release, before (on `/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/s12-before`, do not modify it) and after units 1 and 5, interleaved as in 15.1.

## Rules

- `CARGO_TARGET_DIR=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/target-s12-p2`.
- Nothing named after telnet goes under `crates/`.
- Everything else is in the rules file.

## Final report to the coordinator, at most 12 lines

- units done, with files and test names;
- golden diffs, if any, each explained;
- test counts;
- `todod` times before and after;
- deviations from section 16.
