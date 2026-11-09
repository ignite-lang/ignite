# The terminal system

The reference system of Ignite is a small Telnet command console: a server, `terminald`, and
a client, `tn`. It answers `help`, `echo`, `status` and `quit`. It lives in
[`examples/terminal/`](https://github.com/ignite-lang/ignite/tree/main/examples/terminal), in
the clone of [Get the examples](install.md#get-the-examples). Three files make the system. A
fourth holds the full design declaration:

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

```sh
ignite build examples/terminal/terminal.ignite
```

```text
built target/ignite/terminal/bin/terminald
built target/ignite/terminal/bin/tn
```

Start the server in one terminal and the client in another:

```sh
target/ignite/terminal/bin/terminald
target/ignite/terminal/bin/tn
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

Any Telnet client works too: `telnet 127.0.0.1 2323`. Both programs take `--port N`.
`--port 0` binds a free port, and the server's `listening on` line reports it:

```sh
target/ignite/terminal/bin/terminald --port 0
```

```text
listening on 127.0.0.1:34631
```

With no server on the port, `tn` exits 3:

```text
error: connect 127.0.0.1:1: Connection refused (os error 111)
```

SIGTERM stops `terminald` with exit 0. Every exit status of a compiled executable
is in the [command-line reference](../reference/cli.md#compiled-executables).

Each connection runs in a task of its own. A failed `assert`, an integer overflow or an index
out of range ends that one task and reports where it happened, and the server keeps
accepting. [Failures](../reference/tests.md#failures) gives the report's form.

Next: [an HTTP service](http.md), the todo example.
