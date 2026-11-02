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
| `crates/ignite/` | `ignite` CLI: `check`, `emit`, `build`, `test`; end-to-end tests of the example in `tests/terminal.rs` |
| `crates/ignite-load/` | `ignite-load`: load generator for `terminald` (workloads `rr`, `pipe`, `connect`; `ignite-load matrix` for `task bench-throughput`); not linked into compiled programs |
| `std/` | standard library in Ignite; module `std.a.b` is `std/a/b.ignite` |
| `std/console.ignite` | `Console`, `Reply`, `Action`: a line-oriented console application, independent of the protocol that carries it; `CommandConsole` (the declared form: `implementation: Module<ConsoleCommands>`, `command_schema`), `console_of`, `command_names` |
| `std/text.ignite` | text operations called by UFCS (`split`, `contains`, `to_lower`, `is_digit`, …) |
| `std/option.ignite`, `std/result.ignite` | UFCS helpers on `Option` and `Result` (`unwrap_or`, `map`, `and_then`, `ok_or`, `map_err`, `ok`, …); the shared names make a glob import of both E041 at use |
| `std/json.ignite` | `Json` with integer numbers (RFC 8259): `encode`, `parse` |
| `std/net.ignite` | `Endpoint { address, port }`: the client-side target a bare `TcpListener` resource name gives where `Endpoint` is expected (slice 10); the module is also the substrate builtins `TcpListener`, `TcpStream`, `Exposure` |
| `std/protocol/telnet.ignite` | `protocol Telnet`: server role `serve`, client role `connect`; the executable kinds `TelnetServer` (accept loop, `SessionLimits`, `TelnetProfile`) and `TelnetClient` (exit 3 and 4) with their `main` fns |
| `std/protocol/http.ignite` | `protocol Http`: server role `serve` over `HttpApp<S>`; `Request`, `Response`; the executable kind `HttpServer<S>` |
| `std/contract.ignite`, `std/policy.ignite`, `std/probe.ignite`, `std/deploy.ignite`, `std/cli.ignite` | the declaration vocabulary of design section 11.3 (`SLA`, `Availability`, `Restart`, `DrainRollout`, `TelnetPrompt`, `Deployment`, `Application`, …), checked and tested, not yet run by anything |
| `examples/terminal/terminal.ignite` | the terminal system in component form (`CommandConsole`, `TelnetServer`, `TelnetClient`); `ignite build` emits `terminald` and `tn` |
| `examples/terminal/commands.ignite`, `command_schema.ignite` | sibling modules of the terminal: the console's commands (`respond`) and their names (`commands`), filled into the declaration by `module(..)` |
| `examples/terminal/terminal.design.ignite` | the full declaration from design section 11.3; `ignite check` passes it (`cli::check_design_file_is_silent`), `ignite build` does not build it |
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

## Conventions

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
