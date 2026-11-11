# std.cli

The declaration of a command-line application. Today it is a declaration only: `ignite check`
checks it, and nothing builds or runs it. The kind has no `main`, so it builds no executable.
The buildable client is [`TelnetClient`](telnet.md); a program reads its own arguments through
a [`CommandLine`](process.md#command-line).

- Layer: general.
- Source: [`std/cli.ignite`](https://github.com/ignite-lang/ignite/blob/main/std/cli.ignite).
- Import: `import std.cli.{Application, Command, Argument, endpoint, text}`.

## Types

```ignite,fragment
type Application {
  executable: Text
  commands: Map<Text, Command>
}

type Command {
  arguments: List<Argument> = []
  handler: Module<SessionHandler>
  terminal: Text = "interactive"
  escape: Text = "Ctrl+]"
}

type Argument {
  name: Text
  kind: ArgKind = Plain
}

enum ArgKind {
  Endpoint(Protocol),
  Plain
}

type SessionHandler {
  connect: fn(stream: TcpStream) -> Result<Unit, IoError> requires [stream.*, stdin.*, stdout.*, process.*]
}

fn endpoint(speaking: Protocol) -> ArgKind
fn text() -> ArgKind
```

- `Application` is the executable and its commands by name.
- `Command` is one command. `handler` names the module whose `connect` runs the session over a
  connected stream. `terminal` and `escape` are plain text; nothing checks their value.
- `Argument` is one argument. `endpoint(P)` makes an argument that is a `host:port` of a server
  speaking `P`, `text()` one that is any text.
- `SessionHandler` is what a handler module provides. `module("std.protocol.telnet")` fills it
  with Telnet's `connect`.

The module also exports `describe` and `usage`, which write the usage text of an argument and
of a command.

## In a declaration

The one use is the client of
[`terminal.design.ignite`](https://github.com/ignite-lang/ignite/blob/main/examples/terminal/terminal.design.ignite):

```ignite,fragment
component client: Application {
  executable = "tn"
  commands = {
    connect = Command {
      arguments = [Argument {
        name = "endpoint"
        kind = endpoint(Telnet)
      }]
      handler = module("std.protocol.telnet")
      terminal = "interactive"
      escape = "Ctrl+]"
    }
  }
}
```
