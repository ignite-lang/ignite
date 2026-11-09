<p align="center">
  <img src="docs/assets/logo.svg" alt="Ignite logo: a robot with a flame rising from its head" width="180">
</p>

<h1 align="center">Ignite</h1>

<p align="center">
  <strong>Say what your program is for. One file, one build, one native executable.</strong>
</p>

<p align="center">
  A compiled, intent-oriented language for fast, resource-efficient software development<br>
  by humans and AI agents. One typed file holds the program, its components, its contracts<br>
  and its policies; the compiler turns it into native executables, then runs and checks them.
</p>

---

## What it looks like

An excerpt of the Telnet console system
[`examples/terminal/terminal.ignite`](examples/terminal/terminal.ignite):

```ignite
system Terminal {
  resource ingress: TcpListener { address = "127.0.0.1", port = 2323 }

  component server: TelnetServer {
    listener = ingress
    application = console
    executable = "terminald"
  }
}
```

`ignite build` turns the whole file into two programs, the server `terminald` and the client
`tn`. The book's [terminal walkthrough](https://ignite-lang.github.io/ignite/book/guide/terminal.html)
reads the full program, its commands and its tests.

## Why Ignite

- **Built for many agents compiling at once.** Ignite started as a replacement for Rust, whose
  compile times bring a machine to a halt when several coding agents build at the same time, and
  caches did not fix that. The todo HTTP service builds in 1.30 s on the LLVM target against
  4.12 s through cargo on the Rust target. That is the dev profile at release v0.15.0 on the
  dedicated timing host, from [`quality/trend.md`](quality/trend.md).
- **Specified, then checked.** Ignite is meant as a native realization target for
  [ESS](https://github.com/beyond10x/ess), which turns intent into typed, machine-checkable
  specifications with conformance suites. ESS stays language-independent. Its conformance tests
  would check an Ignite executable like any other. This is planned, see
  [`docs/research/review-2026-11-04-ecosystem.md`](docs/research/review-2026-11-04-ecosystem.md).
- **Intent first.** Sizes, durations and ratios are typed quantities (`4096 bytes`,
  `15 minutes`, `99.9%`). Contracts, restart policies and deployments are declarations the
  compiler checks, not comments next to a YAML file.
- **Protocols are code.** Telnet, HTTP/1.1 and JSON-RPC are written in Ignite, in
  [`std/`](std/). Nothing application-specific is hidden in the runtime.
- **Two native targets.** LLVM is the default, built and linked with clang. Rust
  (`--target rust`) is built with cargo. Both behave the same, down to the text of an error.
- **A failure ends its task, not your server.** A failed `assert`, an integer overflow or an
  out-of-range index stops the one session that hit it and reports the file, line and column.

## Try it

Install `ignite` as the book's [Install](https://ignite-lang.github.io/ignite/book/guide/install.html)
page describes: the pre-built binary of a release on Linux x86_64, the toolchain image, or from
this repository. From a clone, with Rust (stable) and clang:

```bash
cargo install --locked --path crates/ignite
ignite build examples/terminal/terminal.ignite
target/ignite/terminal/bin/terminald     # terminal 1
target/ignite/terminal/bin/tn            # terminal 2
```

```text
terminal> help
help
echo <text>
status
quit
terminal> echo hello
hello
terminal> quit
bye
```

Any Telnet client works too: `telnet 127.0.0.1 2323`. Contributors build and test the
repository with [go-task](https://taskfile.dev): `task build`, `task test`, `task gate`.
[`AGENTS.md`](AGENTS.md) lists every task.

## Status

Ignite is an early prototype. What works today, what is being built and what
is only designed is on the book's
[roadmap](https://ignite-lang.github.io/ignite/book/roadmap.html).

## Read more

The website, [ignite-lang.github.io/ignite](https://ignite-lang.github.io/ignite/), has the
[book](https://ignite-lang.github.io/ignite/book/): a start, how-to guides, the concepts and the
reference. Its source is [`site/`](site/).

| document | what is in it |
|---|---|
| [`docs/design.md`](docs/design.md) | the language design |
| [`docs/vision.md`](docs/vision.md) | direction, architecture and decisions |
| [`docs/slice.md`](docs/slice.md) | the exact contract of what is implemented |
| [`docs/llvm.md`](docs/llvm.md) | how the LLVM target works |
| [`AGENTS.md`](AGENTS.md) | repository layout, commands and conventions |

## Credits

The logo uses the robot and fire emoji from [Noto Emoji](https://github.com/googlefonts/noto-emoji)
by Google, licensed under the Apache License 2.0.
