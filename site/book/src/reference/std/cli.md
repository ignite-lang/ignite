# std.cli

The declaration of a command-line application: a set of commands, each with its arguments and
a handler module. `ignite check` checks these declarations. The kind has no `main`, so it
builds no executable of its own; the buildable client today is
[`TelnetClient`](telnet.md).

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
```

- `Application` is the application: its executable and its commands by name.
- `Command` is one command. `handler` names the module whose `connect` runs the session over a
  connected stream, with the terminal and the process at hand. `terminal` and `escape` are
  closed sets written as text.
- `Argument` is one argument and its kind.
- `SessionHandler` is what a handler module provides. `module("std.protocol.telnet")` fills it
  with Telnet's `connect`.

## Functions

```ignite,fragment
fn endpoint(speaking: Protocol) -> ArgKind
fn text() -> ArgKind
fn describe(name: Text, kind: ArgKind) -> Text
fn usage(name: Text, arguments: List<Argument>) -> Text
```

| function | what it does |
|---|---|
| `endpoint` | an argument that is an endpoint of a server speaking the protocol, written `host:port` |
| `text` | an argument that is any text |
| `describe` | the usage text of one argument: `<name: host:port>` or `<name>` |
| `usage` | the usage line of a command with its arguments |

## In a declaration

From [`terminal.design.ignite`](https://github.com/ignite-lang/ignite/blob/main/examples/terminal/terminal.design.ignite):

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

## Example

```ignite
import std.cli.*

test "usage lines" {
  assert describe("file", text()) == "<file>"
  assert Argument { name = "x" }.kind == Plain
  assert usage("connect", []) == "connect"
  assert usage("copy", [Argument { name = "from" }, Argument { name = "to" }]) == "copy <from> <to>"
}
```
