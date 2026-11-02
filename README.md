<p align="center">
  <img src="docs/assets/logo.svg" alt="Ignite logo: a robot with a flame rising from its head" width="180">
</p>

<h1 align="center">Ignite</h1>

<p align="center">
  <strong>Say what your system is for. Ignite builds it, runs it and keeps it honest.</strong>
</p>

<p align="center">
  An intent-oriented language for specifying, building, deploying and operating systems.<br>
  One file describes the components, the network, the contracts and the policies,<br>
  and the compiler turns it into native executables.
</p>

---

## What it looks like

A Telnet console system, the full program
([`examples/terminal/terminal.ignite`](examples/terminal/terminal.ignite)):

```ignite
system Terminal {
  resource ingress: TcpListener {
    address = "127.0.0.1"
    port = 2323
  }

  component console: CommandConsole {
    implementation = module("examples.terminal.commands")
    prompt = "terminal> "
    maximum_line = 4096 bytes
  }

  component server: TelnetServer {
    listener = ingress
    application = console
    executable = "terminald"
  }

  component client: TelnetClient {
    target = ingress
    executable = "tn"
  }
}
```

`ignite build` turns this into two programs: the server `terminald` and the client `tn`.
The commands themselves are ordinary Ignite code
([`commands.ignite`](examples/terminal/commands.ignite)):

```ignite
fn answer(command: Command) -> Reply = match command {
  Help => Reply { lines = ["help", "echo <text>", "status", "quit"] }
  Echo(text) => Reply { lines = [text] }
  Status => Reply { lines = ["ready"] }
  Quit => Reply { lines = ["bye"], action = Close }
}
```

Tests live next to the code they check:

```ignite
test "slice takes byte offsets" {
  assert slice("hello", 1, 4) == "ell"
  assert "hello".slice(0, 5) == "hello"
}
```

## Why Ignite

- **Intent first.** Sizes, durations and ratios are typed quantities (`4096 bytes`,
  `15 minutes`, `99.9%`). Contracts, restart policies and deployments are declarations the
  compiler checks, not comments next to a YAML file.
- **Protocols are code.** Telnet and HTTP are written in Ignite, in [`std/`](std/). Nothing
  application-specific is hidden in the runtime.
- **Two native targets.** Rust (the default, built with cargo) and LLVM (`--target llvm`,
  built and linked with clang). Both behave the same, down to the text of an error.
- **A failure ends its task, not your server.** A failed `assert`, an integer overflow or an
  out-of-range index stops the one session that hit it and reports the file, line and column:

  ```text
  task ended with error: index failed at <file>:<line>:<column>: index 5 out of range for length 2
  task ended with error: + failed at <file>:<line>:<column>: 9223372036854775807 + 9
  ```

## Try it

You need Rust (stable), [go-task](https://taskfile.dev) and, for the LLVM target, clang.

```bash
task compile      # build terminald and tn from examples/terminal/terminal.ignite
task server       # start terminald on 127.0.0.1:2323
task client       # connect with tn, in a second terminal
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

Any Telnet client works too: `telnet 127.0.0.1 2323`.

| more to try | command |
|---|---|
| the same programs built with LLVM | `task compile-llvm`, then `task server TARGET=llvm` |
| the todo HTTP backend | `task compile-todo`, then `task todo-server` |
| every inline test, both targets | `task test-ignite` |
| the whole test suite | `task test` |

## Status

Ignite is an early prototype. The language, both targets, the standard library and the
system declaration, planning (`ignite plan` and `ignite explain`), running (`ignite run`)
and journal replay (`ignite replay`) work. Applying changes (`ignite apply`) is next. The roadmap is in [`PLAN.md`](PLAN.md).

## Read more

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
