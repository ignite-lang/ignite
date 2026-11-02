# Ignite

Intent-oriented language for specifying, building, deploying and operating systems. Design:
`docs/design.md` (version 0.3, 2026-11-01). Direction and the current slice's contract:
`docs/slice.md`. This repository holds the compiler (Rust), the standard library (Ignite) and
the IO substrate (Rust) that compiled programs link.

## Layout

| path | role |
|---|---|
| `crates/ignite-syntax/` | lexer, parser, AST, diagnostics |
| `crates/ignite-check/` | name resolution across modules, effect and protocol rules; produces `Symbols` |
| `crates/ignite-types/` | type checker: a type for every expression, E040 to E046 (E046 match exhaustiveness) |
| `crates/ignite-rustgen/` | Rust target: the `ignite-std` crate and the program crate |
| `crates/ignite-llvm/` | LLVM target: IR text for std, program and one main per executable |
| `crates/ignite-rt/` | runtime of the LLVM target, `libignite_rt.a`: C ABI adapter over `ignite-io` (value conversion, handle table), arenas and regions, the spawn handshake; no IO of its own |
| `crates/ignite-io/` | IO substrate of both targets, blocking: TCP listener and stream, stdin, stdout, terminal mode, tasks (one thread each), `run`, process arguments and exit, the reports of `assert` and `?`, the test runner (feature `runner`) |
| `crates/ignite-eval/` | compile-time evaluator (`docs/slice.md` section 15): interprets property expressions over the typed modules with fuel (E115, E117, E118), builds the system model `ignite plan` prints, resolves assemblies through `std.deploy`'s `admits` and `includes` (E110 to E114), renders `ignite explain`; steps the reducers of `std.control` for `ignite run` and `ignite replay` (`docs/slice.md` 16.2, O27); used by `check`, `plan`, `explain`, `run` and `replay` only, linked by no backend and no compiled program |
| `crates/ignite/` | `ignite` CLI: `check`, `emit`, `build`, `test`, `plan`, `explain`, `run`, `replay`; end-to-end tests of the example in `tests/terminal.rs`, model and explain goldens in `tests/golden/` |
| `crates/ignite/src/run/` | the driver of `ignite run` and `ignite replay` (`docs/slice.md` 16.3): `engine` steps the `std.control` reducers in `ignite-eval`, `live` runs the adapters `process`, `probe`, `clock` and `control` that turn effects into processes, attempts, timers and stdin commands and feed events back, `journal` records every event, effect, signal, standing and consequence (16.5), `replay` re-steps a journal and compares (E127) |
| `crates/ignite-load/` | `ignite-load`: load generator for `terminald` (workloads `rr`, `pipe`, `connect`; `ignite-load matrix` for `task bench-throughput`); not linked into compiled programs |
| `std/` | standard library in Ignite; module `std.a.b` is `std/a/b.ignite` |
| `std/console.ignite` | `Console`, `Reply`, `Action`: a line-oriented console application, independent of the protocol that carries it; `CommandConsole` (the declared form: `implementation: Module<ConsoleCommands>`, `command_schema`), `console_of`, `command_names` |
| `std/text.ignite` | text operations called by UFCS (`split`, `contains`, `to_lower`, `is_digit`, …) |
| `std/option.ignite`, `std/result.ignite` | UFCS helpers on `Option` and `Result` (`unwrap_or`, `map`, `and_then`, `ok_or`, `map_err`, `ok`, …); the shared names make a glob import of both E041 at use |
| `std/json.ignite` | `Json` with integer numbers (RFC 8259): `encode`, `parse` |
| `std/net.ignite` | `Endpoint { address, port }`: the client-side target a bare `TcpListener` resource name gives where `Endpoint` is expected (slice 10); the module is also the substrate builtins `TcpListener`, `TcpStream`, `Exposure`; `Endpoint.of(ref)` is the substrate builtin that resolves a `Ref` through `ignite run`'s `--ref` arguments (16.9) |
| `std/sli.ignite` | probe attempts, windows, missing data and the meets check; the leaf module that `std.control`, `std.probe` and `std.contract` import |
| `std/time.ignite` | civil time for calendar windows, a leaf: `month_start` and `next_month_start` of an instant in `UTC` or `Europe/Berlin` (EU summer-time rule), `zone_known` |
| `std/control.ignite` | the pure reducers `ignite run` steps (`docs/slice.md` 16.5 to 16.12): `Machine` (`Supervise`, `Schedule`, `Evaluate`, `Roll`), `Event`, `Effect`, `Standing`, `Fired`, `step(machine, event) -> Step`, `companions(machine)`; imports only the leaves `std.sli`, `std.policy`, `std.time` (O45), tested inline with synthetic event lists; `crates/ignite/tests/fixtures/control_trace.ignite` steps the same reducers in the evaluator and on both targets (`control_trace::reducers_agree_across_the_evaluator_and_both_targets`) and times A7 (`control_trace::a7_evaluator_steps_10000_emissions_into_a_30_day_window`) |
| `std/protocol/telnet.ignite` | `protocol Telnet`: server role `serve`, client role `connect`; the executable kinds `TelnetServer` (accept loop, `SessionLimits`, `TelnetProfile`) and `TelnetClient` (exit 3 and 4) with their `main` fns |
| `std/protocol/http.ignite` | `protocol Http`: server role `serve` over `HttpApp<S>`; `Request`, `Response`; the executable kind `HttpServer<S>` |
| `std/contract.ignite`, `std/policy.ignite`, `std/probe.ignite`, `std/deploy.ignite`, `std/cli.ignite` | the declaration vocabulary of design section 11.3 (`SLA`, `Availability`, `Restart`, `DrainRollout`, `TelnetPrompt`, `Deployment`, `Application`, …), checked and tested; `std.deploy`'s `Member`, `admits` and `includes` are what the compiler calls to resolve an assembly (`docs/slice.md` 15.6); since slice 12 each runnable kind has the hooks `machine` and `validate` of 16.16 (`ignite run` steps the machines, `ignite check` reports `validate`'s reasons as E120), `TelnetPrompt.main` is the probe process (16.8) and `std.deploy.entries` names the entries a binding needs (E129) |
| `examples/terminal/terminal.ignite` | the terminal system in component form (`CommandConsole`, `TelnetServer`, `TelnetClient`); `ignite build` emits `terminald` and `tn` |
| `examples/terminal/commands.ignite`, `command_schema.ignite` | sibling modules of the terminal: the console's commands (`respond`) and their names (`commands`), filled into the declaration by `module(..)` |
| `examples/terminal/terminal.design.ignite` | the full declaration from design section 11.3; `ignite check` passes it (`cli::check_design_file_is_silent`); `ignite run` builds it; `task compile` builds `terminal.ignite` |
| `examples/todo/` | the todo system: an HTTP JSON backend over `Store<Todo>`; `ignite build` emits `todod` |
| `docs/vision.md` | direction: target architecture, sequencing of waves and slices, contradictions between documents, open and taken decisions |
| `PLAN.md` | the roadmap in outline: slices, waves, state; reasoning stays in `docs/vision.md` |
| `docs/` | design document, slice contract, `llvm.md` (LLVM target); `friction.md` (language friction backlog); research: `research/lean.md`, `research/ess.md` (comparisons); future designs, not implemented: `wasm.md` (WebAssembly target, runtime in Ignite, hot reload), `actors.md` (typed actors, pub/sub, supervision), `protocols.md` (RFC-bound protocols, pluggable transports) |

Rules of the layout:

- The only hand-written Rust a compiled program runs is `crates/ignite-io` (Rust target) or
  `crates/ignite-rt` over `ignite-io` (LLVM target). Protocols,
  consoles and anything else a program needs are Ignite in `std/` or in the program.
- The substrate is implemented once, in one language, and every target calls the same
  functions. A target differs only in how it suspends a task (Rust futures, LLVM coroutines),
  not in how it does IO. State since slice 4: `crates/ignite-io` is the only substrate, blocking
  (`std::net`, one OS thread per task, no tokio anywhere in the workspace); the Rust target
  calls it directly with synchronous generated code, and `crates/ignite-rt` is only its C ABI
  adapter (no socket, stdin or stdout code of its own). Slice 5 makes it async:
  tokio is the event loop inside `crates/ignite-io`, Rust-target futures run on it directly,
  LLVM-target coroutines run on it as tokio tasks (Timo, 2026-11-01; replaces the libuv
  candidate of design section 8.6).
- `examples/` holds `.ignite` files only.
- Nothing named telnet lives under `crates/`. The substrate knows sockets and TLS sessions, not application protocols: a TLS engine (rustls) may live in `crates/ignite-io`, the `Tls<T>` layer is Ignite (Timo, 2026-11-01).

## Commands

| task | command |
|---|---|
| build everything | `task build` |
| all tests | `task test` |
| format and lint | `task check` |
| merge gate | `task gate` |
| compile the example | `task compile` (writes `target/ignite/terminal/`, prints `built …/terminald` and `built …/tn`) |
| run the compiled server | `task server` |
| run the compiled client | `task client` |
| compile the example with the LLVM target | `task compile-llvm` (writes `target/ignite-llvm/terminal/bin/`) |
| run the LLVM-built binaries | `task server TARGET=llvm`, `task client TARGET=llvm` |
| run the inline Ignite tests | `task test-ignite` (std/ and examples/, both targets; `FILTER=text` selects tests); one file: `target/debug/ignite test FILE [FILTER]` |
| print the system model of a program | `target/debug/ignite plan FILE [--assembly NAME] [--out FILE] [--fuel N]` (canonical JSON, `docs/slice.md` 15.5; the design file's model is the golden `crates/ignite/tests/golden/terminal.design.model.json`) |
| explain one system, member, property or assembly | `target/debug/ignite explain FILE PATH [--assembly NAME]` (`Terminal.reachable`, `Terminal.server.recovery`, `local`; `docs/slice.md` 15.8) |
| run a system in the foreground | `target/debug/ignite run FILE [--assembly NAME] [--state DIR] [--build DIR] [--target rust\|llvm] [--release] [--clock real\|manual] [--epoch RFC3339] [--port RESOURCE=PORT] [--json] [--fresh]` (`docs/slice.md` 16.3; stdin takes `status`, `rollout <member>`, `stop`, `advance <quantity>`; state defaults to `target/ignite-run/<assembly>/`; exit 0 on a requested stop, 1 when a machine gave up, 2 on a diagnostic; `--fuel N` bounds each reducer step only, the plan inside `run` and `replay` keeps the default fuel) |
| replay a run's journal | `target/debug/ignite replay FILE [--assembly NAME] [--state DIR]` (re-steps the reducers over the journaled events, compares every effect and standing record, E127, prints the final standings) |

`task` is go-task. Rust toolchain: stable, edition 2024, `rustfmt` defaults. Every cargo
invocation in automation sets `CARGO_INCREMENTAL=0`. `task test-ignite` honours
`CARGO_TARGET_DIR` (binary, runtime and outputs under it); `task compile-llvm` and
`task compile-todo-llvm` build and pass the runtime under it too, and write their programs to
`target/ignite-llvm/` regardless.

## Gate time

The budget of `docs/vision.md` invariant 9; each slice re-measures and adds its row. 2026-11-01,
slice 9b (one test executable per target, one integration-test binary per crate), 20 cores
shared with other sessions' builds (load average 2 to 18), sccache on, a fresh
`CARGO_TARGET_DIR` after `cargo clean`. Before and after are two copies of one tree that differ
only in 9b. Gate = `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D
warnings`, `cargo test --workspace --no-fail-fast`, `task test-ignite`, run one after another.

| measurement | command | before 9b | after 9b |
|---|---|---|---|
| gate, cold | the gate with `SCCACHE_RECACHE=1` after `cargo clean` | 129.9 s | 86.5 s |
| gate, warm | the gate after `touch crates/ignite-check/src/lib.rs` | 35.4 s | 25.9 s |
| test executables | `cargo test --workspace --no-run` | 49, 666 MB | 18, 202 MB |
| integration-test binaries | same | 39 | 8, one per crate |
| `--no-run`, dependencies cached | same, after `cargo clean` (3 runs before, 2 after) | 3.5 to 18.9 s | 4.6 to 7.0 s |
| `--no-run`, warm | same, repeated | 0.0 to 0.4 s | 0.0 s |
| `--no-run` after a touch | same, after `touch crates/ignite-check/src/lib.rs` | 1.8 to 2.2 s | 1.3 to 1.4 s |
| `task test-ignite` in the cold gate | `task test-ignite` | 12.3 s, 6 executables | 18.4 s, 2 executables |
| `ignite test` of std/ and examples/, cold, Rust | fresh `--out` and `--target-dir`, `SCCACHE_RECACHE=1`, 2 runs | 5.2, 8.4 s | 4.0, 4.1 s |
| same, LLVM | same with `--target llvm` | 3.8, 4.1 s | 2.7, 2.3 s |
| `task test-ignite`, warm | nothing changed | 1.3 s | 0.6 s |

- Test counts, both trees: 520 cargo tests run (518 passed, 2 LLVM goldens failing on slice
  9c's work in progress), 3 ignored; 78 inline tests per target.
- The cold-gate `task test-ignite` row also holds `cargo build -p ignite` and the runtime build;
  how its 6.1 s rise splits between them and load was not measured. Inferred, not measured:
  before 9b the step ran a stale `target/debug/ignite` and skipped the compiler build; it now
  builds `ignite` and the runtime first, so the rise is not a regression of the test build.
- Program builds (steering session, 2026-11-01 18:35, the todo example, cold): Rust target
  3.0 s dev, 3.9 s release; LLVM target 0.8 s dev, 0.9 s release (emit, clang, link);
  `libignite_rt.a` once 3.0 s dev, 4.7 s release.

Slice 10 (component form, 2026-11-02 01:01, P4), one tree, 20 cores shared with other
sessions (load average 11 to 23), sccache on, own `CARGO_TARGET_DIR`; "before" is the
9b column above.

| measurement | command | before (9b) | slice 10 |
|---|---|---|---|
| gate, cold | `cargo clean`, `SCCACHE_RECACHE=1`, the gate | 86.5 s | 72.3 s (fmt 0.9, clippy 7.4, test 55.5, test-ignite 8.5) |
| gate, warm | after `touch crates/ignite-check/src/lib.rs` | 25.9 s | 45.5 s (fmt 0.4, clippy 2.1, test 41.7, test-ignite 1.3) |
| test executables | `cargo test --workspace --no-run` | 18, 202 MB | 18, 210 MB |
| `--no-run`, warm | same, repeated | 0.0 s | 0.1 s |
| `--no-run` after a touch | same, after the touch | 1.3 to 1.4 s | 3.5 s |
| todod, Rust | `ignite build examples/todo/todo.ignite`, fresh `--out` and `--target-dir`, `SCCACHE_RECACHE=1`, 2 runs | 3.0 s dev, 3.9 s release | 4.0 and 3.3 s dev, 4.4 and 4.4 s release |
| todod, LLVM | same with `--target llvm`, 2 runs | 0.8 s dev, 0.9 s release | 0.4 and 0.4 s dev, 1.2 and 1.2 s release |

- Test counts: 552 cargo tests passed, 0 failed, 3 ignored; 97 inline tests per target (78 before;
  the 19 new ones are std/policy 4, contract 3, telnet 3, probe 2, deploy 2, cli 2, console 2,
  examples/terminal/command_schema 1).
- The warm gate rose 76 %: its `cargo test` step is 41.7 s, of which 39.0 s is test execution
  (`crates/ignite` end-to-end binary 33.2 s, `ignite-rustgen` 3.4 s, read from the harness'
  `finished in` lines), not compilation. The end-to-end suite grew in slice 10 (failover, slice10
  crash, llvm_memory, syntax on both targets, siblings, cli, the todo suite), and several of its
  tests build a program with cargo or clang inside the test. Measured at load 21; the 9b column
  was measured at load 2 to 18.
- LLVM release rose 33 %: `clang -O2` on the generated `std.ll` (18,120 lines, every std module)
  takes 0.84 s of the 1.2 s, measured on the build's output; `program.ll` and the main take 0.08 s
  each. Inferred, not measured against a "before" IR (the slice-9 std does not compile with the
  slice-10 compiler): the rise is the std modules slice 10 added (`contract`, `policy`, `probe`,
  `deploy`, `cli`, the kinds in `console`, `telnet`, `http`), which the std module carries whether
  the program uses them or not; `--gc-sections` drops them from the executable but after
  optimisation.
- Rust release rose 13 % and dev 10 to 33 %, with the same inferred cause: the generated
  `ignite-std` crate is 2,126 lines and holds every std module.
- `abi::release_closes_the_socket_and_invalidates_the_handle` (`ignite-rt`) failed once in six
  runs of the suite, in the warm gate: rebinding the released listener's port gave `Address
  already in use`. Inferred, not verified: another process on the host took the ephemeral port
  between release and rebind.

Slice 11 (plan and explain, 2026-11-02 03:00 to 03:10, P4), 20 cores, load average 0.8 to 4.7
throughout, sccache on. Method (`docs/slice.md` 15.1): "before" is a `git archive 60aef38` copy,
"after" a copy of the finished slice-11 tree, each with its own `CARGO_TARGET_DIR`; the cold gate
once per tree in a fresh target dir with `SCCACHE_RECACHE=1`, then three interleaved passes, each
running every warm unit once on the before copy and once on the after copy; the 1-minute load is
recorded per run, a pair whose two loads differ by more than 5 is discarded (none was), and the
ratio is the median of the pair ratios. Warm = after `touch crates/ignite-check/src/lib.rs`.
Scripts and raw logs: the slice's scratch directory, `p4-scripts/` and `p4-logs/`.

| measurement | command | before (60aef38) | slice 11 | ratio |
|---|---|---|---|---|
| gate, cold | fresh target dir, `SCCACHE_RECACHE=1`, the gate (1 pair) | 64.8 s (fmt 0.4, clippy 4.2, test 51.9, test-ignite 8.2) | 54.5 s (0.5, 4.3, 41.6, 8.2) | 0.84 |
| gate, warm | the gate after the touch (3 pairs) | 25.9 s (25.9 to 26.2) | 16.8 s (16.8 to 17.1) | 0.65 |
| warm `cargo test --workspace` step | same (3 pairs) | 22.9 to 23.1 s | 13.5 to 13.8 s | 0.59 |
| warm fmt, clippy, test-ignite steps | same | 0.38, 1.98, 0.65 s | 0.45, 2.19, 0.68 s | 1.16, 1.11, 1.03 |
| test executables | `cargo test --workspace --no-run`, `Executable` lines | 19, 220 MB | 21, 246 MB | |
| `--no-run`, warm / after the touch | same, repeated / after the touch (3 pairs) | 1.01 / 1.45 s | 1.03 / 1.61 s | 1.02 / 1.21 |
| `crates/ignite/tests`, per file | `cargo test -p ignite --test main <file>::` warm (3 pairs) | slice9 17.5, ignite_test 9.1, terminal 2.1, llvm_build 2.1, slice10_llvm 0.7, cli 0.6, failover 0.6 s; the other 7 files 0.6 s together | slice9 3.7, ignite_test 8.6, terminal 2.1, llvm_build 1.5, slice10_llvm 1.0, cli 0.6, failover 0.6; eval 0.4, plan 0.7 (new) | 0.21, 0.95, 1.00, 0.70, 1.22, 0.95, 1.03 |
| todod, Rust dev | `ignite build examples/todo/todo.ignite`, fresh `--out` and `--target-dir`, `SCCACHE_RECACHE=1` (3 pairs); split = separate builds of `ignite-io`, `ignite-std`, the program crate in one fresh target dir | 3.19 s (io 2.37, std 0.37, program 0.26) | 3.19 s (2.36, 0.36, 0.26) | 1.00 |
| todod, Rust release | same with `--release` | 4.12 s (io 2.79, std 0.65, program 0.51) | 4.19 s (2.83, 0.66, 0.51) | 1.02 |
| todod, LLVM dev | same with `--target llvm`; split = the build's clang and link commands rerun by hand | 0.35 s (clang std 0.08, program 0.02, main 0.02, link 0.11) | 0.33 s (0.06, 0.02, 0.02, 0.11) | 0.94 |
| todod, LLVM release | same with `--release` | 1.10 s (clang -O2 std 0.78, program 0.08, main 0.06, link 0.06) | 0.80 s (0.48, 0.08, 0.06, 0.05) | 0.73 |
| `ignite check` of the design file | `ignite check examples/terminal/terminal.design.ignite` (15 pairs) | 0.05 s | 0.06 s | 1.2 |
| `ignite plan`, `ignite explain local` of the design file | `ignite plan …` with `--out`, `ignite explain … local` (15 runs each, load 2.2 to 4.0) | | 0.06 s (0.06 to 0.07), 0.06 s | |

- Test counts, cargo tests plus the inline tests of both targets: 746 passed (552 cargo + 2 × 97
  inline), 3 ignored before; 784 passed (580 cargo + 2 × 102 inline), 4 ignored after
  (`#[ignore]`: the two 100,000-term chain tests, the LLVM `bench`, the new 100,000-term
  lowering); 97 inline tests per target before, 102 after (std/deploy +5). `cargo test
  --workspace` on the working tree at the close: 580 passed, 0 failed, 4 ignored, 13.98 s at load
  3.3; `task gate` 13.4 s warm.
- Slice 10's carry-overs (P0 on 60aef38, raw logs in the scratch directory `p0-logs/`): the warm
  gate is 25.98 s at load under 5 (fmt 0.40, clippy 2.05, test 22.88, test-ignite 0.65) and 75.4 s
  at load 18 to 44, so slice 10's 45.5 s was load, not a regression. Of the warm test step the
  `crates/ignite` end-to-end binary was 18.69 s (82 %): `slice9.rs` 17.4 s serial behind its
  `BUILDS` mutex, of which the inline test `boxed values carried across releases and through the
  store` (`fixtures/slice9/regions.ignite`) was 9.1 s (6.8 s Rust dev, 2.3 s LLVM -O0). todod LLVM
  release 1.16 s: `clang -O2` on `std.ll` 0.834 s (70.5 %), emit 0.113, `program.ll` 0.084, main
  0.064, link 0.063. todod Rust dev 3.04 s (the 3.0 s baseline); release 4.56 s against 3.9 s: libc
  and nix 52 % (recompiled every run under `SCCACHE_RECACHE=1`), `ignite-std` 16 %, the program
  crate 12 %, accepted as the new baseline. The flake
  `failover::a_failing_session_does_not_end_the_service`: 0 failures in 30 runs alone and 30
  beside a full workspace run (P0 on 60aef38); after P3's rewrite (a deadline instead of one read
  after `kill`) again 30/30 alone (P3) and 30/30 beside a workspace run (P4, load 2.5).
- Fixes in slice 11: `slice9.rs` 17.5 s to 3.7 s (the boxed test's sizes 2,000 inserts and 5,000
  links to 500 and 1,000, 6.8 s to 0.31 s Rust and 2.3 s to 0.09 s LLVM with the same assertions;
  `long_chains_lower_quickly` 100,000 to 10,000 terms, 3.5 s to 0.4 s, the 100,000-term lowering
  now `#[ignore]` as `long_chains_100k_lower_quickly`; the 14 audit probes built as two `ignite
  test` groups per target instead of 28 subprocesses, 7.2 s to 1.9 s). `llvm_build.rs` 2.1 to 1.5 s
  and todod LLVM release 1.10 to 0.80 s from P3's reachable-only std emission (`std.ll` 222 to 107
  defines; `clang -O2` on it 0.78 to 0.48 s). The steering target of a warm test step under 8 s is
  not met: 13.5 s at load 3, of which `ignite_test.rs` is 8.6 s (the end-to-end files run in
  parallel, so the binary's wall is its longest file; `timeout_kills_a_test` 2.6 s,
  `overflow_fails_the_same_everywhere` 1.7 s, each spawning `ignite test` per target and profile),
  the rebuild after the touch 1.6 s, the other crates' binaries about 3 s.
- Slice 11b (2026-11-02, the four target bugs and the warm test step; 3 interleaved passes over
  the 60aef38 copy, the slice-11 close tree and the 11b tree, load 6 to 9): warm `cargo test
  --workspace` step 10.8 s median (10.3 to 17.8 s) against 16.1 s for the slice-11 tree in the
  same passes (ratio 0.70); `ignite_test.rs` 8.6 s to 4.6 s (the two targets on two threads, one
  shared basics run). The target "under 8 s at load under 5" was not checked: the load never fell
  under 5. Remainder: the end-to-end binary 5.6 s, the rebuild after the touch 1.8 s, the
  rustgen tests 1.7 s. todod against the 60aef38 copy in the same passes: Rust dev 3.35 s
  (ratio 1.00), release 4.74 s (1.03); LLVM dev 0.35 s (0.95), release 0.88 s (0.70). Cargo
  tests 582 passed, 0 failed, 4 ignored; inline tests 102 per target.
- Rises over 10 %: fmt +16 % (70 ms) and clippy +11 % (0.2 s) from the new crate `ignite-eval`;
  `--no-run` after the touch +21 % (0.16 s) because `ignite-eval` depends on `ignite-types` and
  recompiles with it; `ignite check` of the design file +20 % (10 ms, at 10 ms resolution): O20
  evaluates every property and resolves the assembly at check time; `slice10_llvm.rs` +22 %
  (0.69 to 0.98 s, after-tree range 0.69 to 1.00 s: one slow run, load noise, inferred); test
  executables 19 to 21 (+12 % size): `ignite-eval`'s unit and integration test binaries.
- Flake seen in the measurement: `llvm_build::default_runtime_is_next_to_the_driver_or_in_its_parent`
  failed once per tree in 8 gate runs with `Text file busy` at the exec of the copied driver (the
  copy's write descriptor inherited by another test thread's fork for an instant); the test now
  retries the exec on ETXTBSY. Pre-existing on 60aef38, not a slice-11 regression.

Slice 12 (`ignite run`, 2026-11-02 06:30 to 06:43, P4), 20 cores shared with other sessions'
test runs (load average 6 to 28 throughout; 3 to 6 for the before tree's cold gate at 05:43),
sccache on. Method as slice 11 (`docs/slice.md` 16.1): "before" is the coordinator's
`s12-before` copy of the slice-12 start tree, "after" a copy of the finished tree, each with its
own `CARGO_TARGET_DIR`; one cold gate per tree, then three interleaved passes; a pair whose two
loads differ by more than 5 or whose step failed is discarded. Scripts and raw logs: the slice's
scratch directory, `s12-p4-scripts/` and `s12-p4-logs/measure/` (`pairs.txt` is the summary).

| measurement | command | before (s12-before) | slice 12 | ratio |
|---|---|---|---|---|
| gate, cold | fresh target dir, `SCCACHE_RECACHE=1`, the gate (1 pair, loads 3.1 and 6.5) | 57.1 s (fmt 0.47, clippy 4.61, test 43.54, test-ignite 8.45) | 80.6 s (0.53, 5.06, 60.02 with 4 failures, 14.96) | 1.41 |
| gate, warm | the gate after the touch (2 pairs kept, loads 11 to 25) | 13.9 s (13.7 to 14.1) | 30.8 s (19.4 to 42.1) | 2.22 |
| warm `cargo test --workspace` step | same (all 4 pairs discarded: exit 101 after) | 10.2, 10.7, 20.9 s at load 11, 25, 17 | 14.9, 15.7, 35.3 s at load 21, 25, 11 | |
| warm fmt, clippy, test-ignite steps | same (3 pairs each) | 0.47, 2.33, 0.71 s | 0.52, 3.86, 1.12 s | 1.13, 1.25, 1.67 |
| test executables | `cargo test --workspace --no-run`, `Executable` lines | 21, 247 MB | 21, 265 MB | |
| `--no-run`, warm / after the touch | same (3 pairs) | 1.13 / 1.89 s | 1.29 / 2.30 s | 1.03 / 1.26 |
| `crates/ignite/tests`, per file | `cargo test -p ignite --test main <file>::` warm (3 pairs, loads 15 to 27) | ignite_test 6.6, slice9 3.9, llvm_build 2.5, terminal 2.1, slice10_llvm 1.4, plan 1.0, failover 0.9, cli 0.7, llvm_memory 0.6, eval 0.5 s | ignite_test 9.8, slice9 6.0, llvm_build 3.6, terminal failed, slice10_llvm 1.5, plan 2.3, failover 1.7, cli 0.7, llvm_memory 0.9, eval 0.8; new: run 3.0 (2.9 to 3.1), drain 1.9 (1.7 to 2.2), control_trace 0.7, decode 0.2 | 1.47, 1.50, 1.45, , 1.52, 2.46, 1.39, 0.90, 1.53, 1.54 |
| todod, Rust dev / release | `ignite build examples/todo/todo.ignite`, fresh `--out` and `--target-dir`, `SCCACHE_RECACHE=1` (3 pairs, loads 12 to 28) | 4.1 to 8.2 s / 4.5 to 9.2 s | 3.9 to 8.4 s / 5.2 to 10.5 s | 1.02 / 1.15 |
| todod, LLVM dev / release | same with `--target llvm` | 0.34 to 0.77 s / 0.82 to 2.02 s | 0.51 to 1.26 s / 1.01 to 2.27 s | 1.64 / 1.20 |
| same, P2 after unit 1 at load 1 to 4 | `s12-p2-bench-unit1cold/todod.log`, 3 pairs | Rust 3.32 / 4.25 s, LLVM 0.33 / 0.81 s | Rust 3.50 / 4.76 s, LLVM 0.50 / 0.98 s | 1.07 / 1.12, 1.52 / 1.23 |
| `ignite check`, `ignite plan` of the design file | 15 pairs each | 0.06 s, 0.06 s | 0.12 s, 0.12 s | 1.83, 1.85 |
| `ignite run` to the first `ready` | P3, `p3-measure/run-ready-*`, warm build, manual clock, 5 runs | | 127 ms | |
| `ignite replay` of a 10,071-record journal | P3, `p3-measure/replay-*`, 5 runs | | 2.54 to 2.63 s | |
| A7: 10,000 emissions into a 30-day rolling window, evaluator | `control_trace::a7_evaluator_steps_10000_emissions_into_a_30_day_window` (`#[ignore]`; the gate runs the 1,000-step variant) | | 12.5 s, 1.25 ms per step | |

- Test counts: before 786 passed (582 cargo + 2 × 102 inline), 0 failed, 4 ignored; after 932 run
  (636 cargo + 2 × 148 inline), 5 ignored (A7 10,000 added): 928 passed and 4 failed in the
  measured snapshot, 932 passed at the close (below). Inline tests per target 102 to 148
  (std/control 28, time 5, sli +1, contract +2, telnet +4, probe, deploy +1, policy +2).
- The measured snapshot's 4 failures were deterministic (3 of 3 reruns alone at load 13.6), no
  load flake, and are fixed at the close. `terminal::idle_terminald_runs_one_thread` and
  `terminal::{rust,llvm}::a_task_that_cannot_start_closes_only_its_connection`: the drain's
  `sigwait` thread (comm `drain`, `crates/ignite-io/src/drain.rs`, 16.13) is a second idle
  thread and counts against systemd's `TasksMax=2`, so the first session's thread got EAGAIN
  (P2, verified); the tests now expect 2 threads idle and 3 with a session and start the server
  under `TasksMax=3` (`crates/ignite/tests/terminal.rs` only, no timed path).
  `terminal::terminal_checks_clean_against_the_standard_library`
  (`crates/ignite-check/tests/terminal.rs:100`) listed the terminal program's functions before
  the 16.16 hooks existed; it now expects `TelnetServer.machine` and `TelnetServer.validate`
  beside the two mains. The working tree's gate at the close (06:47, load 10 to 24): fmt 0.51 s
  exit 0, clippy 0.84 s exit 0, `cargo test --workspace --no-fail-fast` 34.5 s exit 0 (636 passed,
  0 failed, 5 ignored), `task test-ignite` 2.30 s exit 0 (148 per target), `task gate` 28.1 s
  exit 0, `task compile` 0.47 s, `compile-llvm` 1.00 s, `compile-todo` 0.43 s, `compile-todo-llvm`
  0.47 s, all exit 0 (`s12-p4-logs/measure/finalgate.log.summary`).
- The warm test step's target (under 10.8 s, slice 11b) is not met in this measurement and not
  decided by it: the before tree itself took 10.2 to 20.9 s at load 11 to 25 (10.3 to 10.8 s at
  load 6 to 9 in 11b), and the after pairs failed. Measured growth: `ignite_test.rs` 6.6 to 9.8 s
  sets the end-to-end binary's wall (its files run in parallel), the new `run.rs` 3.0 s and
  `drain.rs` 1.9 s are under O41's 6 s together, `plan.rs` 1.0 to 2.3 s. The 35.3 s after run is
  pass 2, during which the load rose from 10.7 to 24.8 (inferred: load, the other two after runs
  are 14.9 and 15.7 s). A re-measurement at load under 5 is owed.
- Rises over 10 %, measured causes: `ignite check` and `ignite plan` of the design file doubled
  (0.06 to 0.12 s, 15 pairs; P3: the std the CLI loads grew from 3,033 to 5,135 lines and model
  construction stays about 1 ms in both trees, `p3-measure/plan-*.log`). Every test that spawns
  the CLI pays that 60 ms per spawn, which is the growth of the short per-file rows (plan,
  siblings, syntax, todo, llvm, eval: 0.2 to 1.3 s more each) and of warm `task test-ignite`
  (0.71 to 1.12 s, also 102 to 148 inline tests); the split between the two is inferred. Program
  builds pay the larger generated std: `ignite-std` 2,191 to 3,792 lines (P2, measured) for the
  Rust rows (todod Rust 1.07 / 1.12 at low load; ignite_test, slice9, llvm_build 1.45 to 1.50,
  which build programs inside the tests, inferred); for LLVM the frontend on the larger std (P2:
  check 0.058 to 0.102 s measured, `clang -O2` on `std.ll` unchanged at 0.50 s and 107 defines
  measured, IR emission not measured, so the LLVM dev remainder is inferred). fmt +13 % (50 ms),
  clippy +25 % (1.5 s, range 1.10 to 1.66), `--no-run` after the touch +26 % (0.4 s): the new
  `crates/ignite/src/run/` (9 modules), `ignite-io` `drain` and `clock`, and the new test files
  (inferred from the source growth, not split). Executables +7 % in size at the same count.
- Cold gate 1.41: 60.0 s against 43.5 s in the test step holds 142 more tests, among them the
  end-to-end `run` and `drain` suites that build the design file once per target with cargo
  inside the test (P3: builds shared per target, O41), at load 6.7 to 9.4 against 3.8 to 6.0
  (inferred split); cold `task test-ignite` 8.5 to 15.0 s at load 9.4 to 12.5 (148 tests, the
  compiler build inside the step; inferred).

- No comments in `Cargo.toml`, `Taskfile.yml` or any other config file.
- Reasoning lives in doc comments, `docs/slice.md` and commit messages, not in reports.
- `docs/slice.md` is the contract between the compiler crates. Change it and the tests together;
  do not let a crate's doc comment restate it.
- Public APIs of the compiler crates are the frozen skeletons' shapes; add, do not rename.
- Generated Rust is an artifact: never edit it, never commit it. `target/ignite/` holds it.
- Tests exercise behaviour through public APIs: the parser through source text, the checker
  through fixtures with `.expected` diagnostics, the generator through golden files and a real
  `cargo build`, the compiled programs through a real socket and real stdin/stdout.
- Each diagnostic code in `docs/slice.md` section 8 has a fixture or test that triggers it.
- Do not add a dependency without a line in the commit message saying what it replaces.
- Ignite behaviour is tested inline with `test` blocks; Rust tests cover the compiler and end-to-end binaries.
- One integration-test binary per crate (`docs/vision.md` invariant 9): every file in a crate's
  `tests/` is a module of its `tests/main.rs` (`autotests = false`, one `[[test]] name = "main"`).
  A new test file gets a `mod` line there, never a `[[test]]` of its own. Shared helpers are
  `tests/common/mod.rs`, declared in `main.rs` and imported with `use crate::common;`. Harness
  names are `<file>::<test>`: one file runs with `cargo test -p <crate> --test main <file>::`.
- `ignite test` links one test executable per target for all files given (`docs/slice.md`
  12.7); only two programs with one module path get separate executables.

## Lifecycle ownership

- `ignite build` owns the generated project directory it was given; it overwrites it.
- `ignite-io::run` owns the process entry and `ignite-io::spawn` the task threads; generated `main`
  never starts a thread itself.
