# The terminal walkthrough

The reference system of Ignite is a small Telnet command console: a server, `terminald`, and
a client, `tn`. It answers `help`, `echo`, `status` and `quit`. It lives in
[`examples/terminal/`](https://github.com/ignite-lang/ignite/tree/main/examples/terminal) and
is split over three files, and a fourth holds the full design declaration:

| file | what it holds |
|---|---|
| `terminal.ignite` | the system: a listener, a console, the server and the client |
| `commands.ignite` | the console's commands, plain Ignite functions, and their tests |
| `command_schema.ignite` | the command names, for tooling |
| `terminal.design.ignite` | the full declaration with a probe, contracts, policies and an assembly ([next chapters](operate.md)) |

## The system

```ignite,dir=examples/terminal
package examples.terminal

import std.console.CommandConsole
import std.protocol.telnet.{TelnetServer, TelnetClient}

system Terminal {
  resource ingress: TcpListener {
    address = "127.0.0.1"
    port = 2323
  }

  component console: CommandConsole {
    implementation = module("examples.terminal.commands")
    prompt = "terminal> "
    maximum_line = 4096 bytes
    command_schema = module("examples.terminal.command_schema")
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

Read it top to bottom:

- **`resource ingress: TcpListener`** is a listening socket on `127.0.0.1:2323`. A resource is
  an owned external thing with a lifecycle; the generated `main` binds it.
- **`component console: CommandConsole`** is the console application. Its commands come from
  the sibling module `examples.terminal.commands`, named with `module("…")`. The maximum line
  is a typed quantity, `4096 bytes`, not a bare number.
- **`component server: TelnetServer`** serves the console over Telnet on the listener. The kind
  `TelnetServer` lives in `std.protocol.telnet`; its `main` is the accept loop, written in
  Ignite, which runs one task per connection.
- **`component client: TelnetClient`** connects to the same resource. Where an endpoint is
  expected, the bare name `ingress` gives its address and port.

`CommandConsole` has no `main`, so it produces no executable: it is a value embedded where
`server` names it. `TelnetServer` and `TelnetClient` have one each, so the system produces
two executables, named by their `executable` fields.

## The commands

`commands.ignite` turns an input line into a reply. This is the core of it:

```ignite
import std.console.{Reply, Action, first_word, rest_after_word}

enum Command {
  Help,
  Echo(Text),
  Status,
  Quit
}

fn answer(command: Command) -> Reply = match command {
  Help => Reply { lines = ["help", "echo <text>", "status", "quit"] }
  Echo(text) => Reply { lines = [text] }
  Status => Reply { lines = ["ready"] }
  Quit => Reply { lines = ["bye"], action = Close }
}

fn parse(line: Text) -> Option<Command> = match first_word(line) {
  "help" => Some(Help)
  "status" => Some(Status)
  "quit" => Some(Quit)
  "echo" => Some(Echo(rest_after_word(line)))
  _ => None
}

test "parse reads the four commands by their first word" {
  assert parse("help") == Some(Help)
  assert parse("echo hello world") == Some(Echo("hello world"))
  assert parse("HELP") == None
}
```

The real file adds `respond(line: Text) -> Reply`, which trims the line, ignores blank
lines and names unknown commands. Its public `respond` is what the console's
`implementation = module(…)` binding fills in: `std.console.ConsoleCommands` is a record
with one field, `respond: fn(Text) -> Reply`.

Nothing here knows about Telnet. The protocol module shows the prompt, cuts the input into
lines of at most `maximum_line` bytes, calls `respond` and renders the reply. `Reply` has a
`lines` list and an `action`, `Continue` by default or `Close` to end the session.

## Build and run

From the clone:

```sh
ignite build examples/terminal/terminal.ignite
```

```text
built target/ignite/terminal/target/debug/terminald
built target/ignite/terminal/target/debug/tn
```

Start the server in one terminal and the client in another:

```sh
target/ignite/terminal/target/debug/terminald
target/ignite/terminal/target/debug/tn
```

The server prints `listening on 127.0.0.1:2323`; the client shows the prompt:

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

Any Telnet client works too: `telnet 127.0.0.1 2323`. Both programs take `--port N`;
`--port 0` binds a free port and the server's `listening on` line reports it. Inside the
clone, `task server` and `task client` do the build and the start in one step.

The executables' exit statuses:

| status | meaning |
|---|---|
| 0 | `main` returned normally |
| 1 | binding the listener failed, or `main` returned an error without an exit code of its own |
| 2 | `--port` malformed or out of range |
| 3 | `tn`: connecting to the server failed |
| 4 | `tn`: the Telnet session ended with an error |

## A failure ends its task, not the server

Each connection runs in a task of its own. A failed `assert`, an integer overflow or an
index out of range ends that one task and reports where it happened; the server keeps
accepting:

```text
task ended with error: index failed at <file>:<line>:<column>: index 5 out of range for length 2
```

## The same transcript without a socket

`commands.ignite` ends with a test that runs the whole Telnet session in-process.
`Pipe.pair()` gives two connected in-memory streams, and `Telnet.serve` is generic over any
`ByteStream`, so the test talks to the real protocol code without opening a port. Shortened,
it reads:

```ignite,fragment
test "the terminal transcript over a pipe" {
  let (client, server) = Pipe.pair()
  client.set_idle_timeout(5000)?
  spawn Telnet.serve(server, SessionLimits {}, Console { prompt = "terminal> ", respond = respond, maximum_line = 4096 })
  assert read_once(client)? == "terminal> "
  client.write_all(b"help\r\n")?
  assert read_once(client)? == "help\r\necho <text>\r\nstatus\r\nquit\r\nterminal> "
  client.write_all(b"quit\r\n")?
  assert read_once(client)? == "bye\r\n"
}
```

Run the example's tests (`--target all` adds the LLVM target; [Testing](testing.md#both-targets)
shows the runtime archive it needs):

```sh
ignite test examples/terminal/commands.ignite
```

## With the LLVM target

```sh
cargo build -p ignite-rt
ignite build examples/terminal/terminal.ignite --target llvm --out target/ignite-llvm/terminal \
  --runtime target/debug/libignite_rt.a
target/ignite-llvm/terminal/bin/terminald
```

The programs behave the same; the binaries are in `target/ignite-llvm/terminal/bin/`
(`task server TARGET=llvm` and `task client TARGET=llvm` do the same in the clone). See
[Building for Rust and LLVM](targets.md).

Next: [testing with ignite test](testing.md), then
[plan, explain, run and replay](operate.md) with the full declaration.
