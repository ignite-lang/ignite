# Telos

Telos is an intent-oriented language for specifying, building, deploying and operating
systems. The design is in [`docs/Telos_Design.md`](docs/Telos_Design.md); the contract of the
current implementation slice is in [`docs/slice.md`](docs/slice.md).

This repository holds the Telos compiler, written in Rust, and the Telos standard library,
written in Telos (`std/`). Two targets: Rust (default, built with cargo) and LLVM
(`--target llvm`: LLVM IR built and linked by clang, no cargo or rustc; contract in
[`docs/llvm.md`](docs/llvm.md)). Compiled programs link the generated
standard library and one hand-written IO substrate (`crates/telos-io`: TCP streams, stdin and
stdout). Protocols are declared in Telos: Telnet is `std/protocol/telnet.telos`.

The example system is a Telnet command console:
[`examples/terminal/terminal.telos`](examples/terminal/terminal.telos). `telos build` turns it
into two executables, the server `terminald` and the client `tn`.

## Try it

```bash
task compile           # telos build examples/terminal/terminal.telos
task server            # target/debug/terminald, listening on 127.0.0.1:2323
task client            # target/debug/tn, in a second terminal
```

LLVM target: `task compile-llvm`, then `task server TARGET=llvm` and
`task client TARGET=llvm` (binaries in `target/telos-llvm/terminal/bin/`).

```text
terminal> help
help
echo <text>
status
quit
terminal> echo hello
hello
terminal> status
ready
terminal> quit
bye
```

Any Telnet client works too: `telnet 127.0.0.1 2323`. `task test` runs the parser, checker,
generator, substrate and end-to-end suites; the end-to-end suite compiles the example and
drives the compiled `terminald` and `tn`.

## Layout

See [`AGENTS.md`](AGENTS.md).
