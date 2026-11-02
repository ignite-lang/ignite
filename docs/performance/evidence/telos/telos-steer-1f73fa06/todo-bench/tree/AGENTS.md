# Telos

Intent-oriented language for specifying, building, deploying and operating systems. Design:
`docs/Telos_Design.md` (version 0.3, 2026-11-01). Direction and the current slice's contract:
`docs/slice.md`. This repository holds the compiler (Rust), the standard library (Telos) and
the IO substrate (Rust) that compiled programs link.

## Layout

| path | role |
|---|---|
| `crates/telos-syntax/` | lexer, parser, AST, diagnostics |
| `crates/telos-check/` | name resolution across modules, effect and protocol rules; produces `Symbols` |
| `crates/telos-types/` | type checker: a type for every expression, E040 to E046 (E046 match exhaustiveness) |
| `crates/telos-rustgen/` | Rust target: the `telos-std` crate and the program crate |
| `crates/telos-llvm/` | LLVM target: IR text for std, program and one main per executable |
| `crates/telos-rt/` | runtime of the LLVM target, `libtelos_rt.a`: C ABI adapter over `telos-io` (value conversion, handle table), arenas and regions, the spawn handshake; no IO of its own |
| `crates/telos-io/` | IO substrate of both targets, blocking: TCP listener and stream, stdin, stdout, terminal mode, tasks (one thread each), `run`, process arguments and exit, the reports of `assert` and `?`, the test runner (feature `runner`) |
| `crates/telos/` | `telos` CLI: `check`, `emit`, `build`, `test`; end-to-end tests of the example in `tests/terminal.rs` |
| `crates/telos-load/` | `telos-load`: load generator for `terminald` (workloads `rr`, `pipe`, `connect`; `telos-load matrix` for `task bench-throughput`); not linked into compiled programs |
| `std/` | standard library in Telos; module `std.a.b` is `std/a/b.telos` |
| `std/console.telos` | `Console`, `Reply`, `Action`: a line-oriented console application, independent of the protocol that carries it |
| `std/text.telos` | text operations called by UFCS (`split`, `contains`, `to_lower`, `is_digit`, …) |
| `std/json.telos` | `Json` with integer numbers (RFC 8259): `encode`, `parse` |
| `std/protocol/telnet.telos` | `protocol Telnet`: server role `serve`, client role `connect` |
| `std/protocol/http.telos` | `protocol Http`: server role `serve` over `HttpApp<S>`; `Request`, `Response` |
| `examples/terminal/terminal.telos` | the terminal system; `telos build` emits `terminald` and `tn` |
| `examples/terminal/terminal.design.telos` | the full declaration from design section 11.3, not compilable yet |
| `examples/todo/` | the todo system: an HTTP JSON backend over `Store<Todo>`; `telos build` emits `todod` |
| `docs/vision.md` | direction: target architecture, sequencing of waves and slices, contradictions between documents, open and taken decisions |
| `docs/` | design document, slice contract, `llvm.md` (LLVM target); `friction.md` (language friction backlog); research: `research/lean.md`, `research/ess.md` (comparisons); future designs, not implemented: `wasm.md` (WebAssembly target, runtime in Telos, hot reload), `actors.md` (typed actors, pub/sub, supervision), `protocols.md` (RFC-bound protocols, pluggable transports) |

Rules of the layout:

- The only hand-written Rust a compiled program runs is `crates/telos-io` (Rust target) or
  `crates/telos-rt` over `telos-io` (LLVM target). Protocols,
  consoles and anything else a program needs are Telos in `std/` or in the program.
- The substrate is implemented once, in one language, and every target calls the same
  functions. A target differs only in how it suspends a task (Rust futures, LLVM coroutines),
  not in how it does IO. State since slice 4: `crates/telos-io` is the only substrate, blocking
  (`std::net`, one OS thread per task, no tokio anywhere in the workspace); the Rust target
  calls it directly with synchronous generated code, and `crates/telos-rt` is only its C ABI
  adapter (no socket, stdin or stdout code of its own). Slice 5 makes it async:
  tokio is the event loop inside `crates/telos-io`, Rust-target futures run on it directly,
  LLVM-target coroutines run on it as tokio tasks (Timo, 2026-11-01; replaces the libuv
  candidate of design section 8.6).
- `examples/` holds `.telos` files only.
- Nothing named telnet lives under `crates/`. The substrate knows sockets and TLS sessions, not application protocols: a TLS engine (rustls) may live in `crates/telos-io`, the `Tls<T>` layer is Telos (Timo, 2026-11-01).

## Commands

| task | command |
|---|---|
| build everything | `task build` |
| all tests | `task test` |
| format and lint | `task check` |
| merge gate | `task gate` |
| compile the example | `task compile` (writes `target/telos/terminal/`, prints `built …/terminald` and `built …/tn`) |
| run the compiled server | `task server` |
| run the compiled client | `task client` |
| compile the example with the LLVM target | `task compile-llvm` (writes `target/telos-llvm/terminal/bin/`) |
| run the LLVM-built binaries | `task server TARGET=llvm`, `task client TARGET=llvm` |
| run the inline Telos tests | `task test-telos` (std/ and examples/, both targets; `FILTER=text` selects tests); one file: `target/debug/telos test FILE [FILTER]` |

`task` is go-task. Rust toolchain: stable, edition 2024, `rustfmt` defaults. Every cargo
invocation in automation sets `CARGO_INCREMENTAL=0`.

## Conventions

- No comments in `Cargo.toml`, `Taskfile.yml` or any other config file.
- Reasoning lives in doc comments, `docs/slice.md` and commit messages, not in reports.
- `docs/slice.md` is the contract between the compiler crates. Change it and the tests together;
  do not let a crate's doc comment restate it.
- Public APIs of the compiler crates are the frozen skeletons' shapes; add, do not rename.
- Generated Rust is an artifact: never edit it, never commit it. `target/telos/` holds it.
- Tests exercise behaviour through public APIs: the parser through source text, the checker
  through fixtures with `.expected` diagnostics, the generator through golden files and a real
  `cargo build`, the compiled programs through a real socket and real stdin/stdout.
- Each diagnostic code in `docs/slice.md` section 8 has a fixture or test that triggers it.
- Do not add a dependency without a line in the commit message saying what it replaces.
- Telos behaviour is tested inline with `test` blocks; Rust tests cover the compiler and end-to-end binaries.

## Lifecycle ownership

- `telos build` owns the generated project directory it was given; it overwrites it.
- `telos-io::run` owns the process entry and `telos-io::spawn` the task threads; generated `main`
  never starts a thread itself.
