# std.console

A line-oriented console application, independent of the protocol that carries it. A protocol
such as [std.protocol.telnet](telnet.md) shows the console's `prompt`, cuts the input into
lines of at most `maximum_line` bytes, passes each line to `respond` and renders the reply. The
module also holds the declared form a `system` uses, `CommandConsole`, whose commands come
from a module of your program.

- Layer: general.
- Source: [`std/console.ignite`](https://github.com/ignite-lang/ignite/blob/main/std/console.ignite).
- Import: `import std.console.{Console, Reply, Action, first_word, rest_after_word}`.

## The runtime console

```ignite,fragment
type Console {
  prompt: Text
  respond: fn(Text) -> Reply
  maximum_line: Int
}

enum Action { Continue, Close }

type Reply {
  lines: List<Text>
  action: Action = Continue
}
```

- `Console` is the value a protocol runs: the prompt, the function that answers one line, and
  the longest line in bytes.
- `Reply` is the answer to one line: the lines to show, and whether the session goes on.
  `action` defaults to `Continue`; `Close` ends the session after the reply.
- `respond` is a field of function type, so `console.respond(line)` calls it.

## The declared console

```ignite,fragment
type ConsoleCommands {
  respond: fn(Text) -> Reply
}

type CommandSchema {
  commands: fn() -> List<Text>
}

type CommandConsole {
  implementation: Module<ConsoleCommands>
  prompt: Text
  maximum_line: ByteSize = 4 KiB
  command_schema: Option<Module<CommandSchema>> = None
}
```

- `ConsoleCommands` is what a console's command module provides: a public `respond` function.
- `CommandSchema` is what an optional schema module provides: the command names, for tooling.
- `CommandConsole` is the component kind a `system` declares. `implementation =
  module("examples.terminal.commands")` fills `ConsoleCommands` from the public functions of
  that module. It has no `main`, so it produces no executable; a server embeds it.

```ignite,fragment
component console: CommandConsole {
  implementation = module("examples.terminal.commands")
  prompt = "terminal> "
  maximum_line = 4096 bytes
  command_schema = module("examples.terminal.command_schema")
}
```

## Functions

```ignite,fragment
fn console_of(declared: CommandConsole) -> Console
fn command_names(declared: CommandConsole) -> List<Text>
fn first_word(line: Text) -> Text
fn rest_after_word(line: Text) -> Text
fn skip_space(bytes: Bytes, i: Int) -> Int
fn skip_word(bytes: Bytes, i: Int) -> Int
```

| function | what it does |
|---|---|
| `console_of` | the runtime `Console` of a declared one; the byte size becomes an `Int` |
| `command_names` | the names of the schema module's commands, none without a schema |
| `first_word` | the first whitespace-separated word; leading whitespace is skipped |
| `rest_after_word` | the text after the first word and one following whitespace byte |
| `skip_space` | the index of the first non-whitespace byte at or after `i` |
| `skip_word` | the index of the first whitespace byte at or after `i` |

## Example

```ignite
import std.console.*

fn shout(line: Text) -> Reply = Reply { lines = ["{line}!"] }

test "words of a command line" {
  assert first_word("  echo hello") == "echo"
  assert rest_after_word("echo hello") == "hello"
  assert rest_after_word("echo  x") == " x"
  assert first_word("") == ""
}

test "a reply continues unless it says close" {
  assert Reply { lines = ["hi"] }.action == Continue
  assert Reply { lines = [], action = Close }.action == Close
}

test "a declared console gives the console a protocol runs" {
  let declared = CommandConsole {
    implementation = ConsoleCommands { respond = shout }
    prompt = "> "
    maximum_line = 8 bytes
  }
  let console = console_of(declared)
  assert console.prompt == "> "
  assert console.maximum_line == 8
  assert console.respond("hi").lines == ["hi!"]
  assert command_names(declared) == []
}
```

The terminal example's
[`commands.ignite`](https://github.com/ignite-lang/ignite/blob/main/examples/terminal/commands.ignite)
is a complete command module: it parses each line with `first_word` and `rest_after_word`
and answers with a `Reply`.
