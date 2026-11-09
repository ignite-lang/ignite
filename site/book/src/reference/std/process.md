# std.process

Child processes, random bytes, launch arguments and environment variables. Each is a
capability: a handle that a system declares as a resource and passes to the code that may use
it. A function reaches a child process, a flag or a variable only through a value it was given.

- Layer: base (substrate).
- Source: [`std/process.ignite`](https://github.com/ignite-lang/ignite/blob/main/std/process.ignite)
  declares the records `Invocation` and `ChildExit`, the enum `Ending` and two functions. The
  capability types and their methods are builtins of the substrate, specified in
  [`docs/slice.md`](https://github.com/ignite-lang/ignite/blob/main/docs/slice.md) sections 21.3
  and 21.5 to 21.7, 21.19 and 21.20.
- Import: the builtin names need none. The declared items need one:
  `import std.process.{Invocation, ChildExit, Ending, outcome, succeeded}`.

The process's own operations, `Clock.sleep`, `Clock.now` and `Process.flush`, are on
[std.io](io.md).

## Resources

```ignite,fragment
resource tools: Launcher { maximum_children = 16 }
resource entropy: Entropy
resource tool: Setting { flag = "--tool", default = "/bin/sh" }
resource command_line: CommandLine
resource environment: Environment
```

| resource | property | default | meaning |
|---|---|---|---|
| `Launcher` | `maximum_children` | 16 | the most children started and not yet waited for, at once; 1 to 1024 (E104) |
| `Setting` | `flag` | required (E104) | the flag, `--` followed by `[a-z0-9-]+` |
| `Setting` | `default` | absent | the value when the flag is not given; absent makes the flag required at launch |
| `Entropy`, `CommandLine`, `Environment` | none | | any property is E047 |

A capability resource needs no deployment binding: it belongs to the process of the member
that uses it. A component receives it through a property, typically the `state` of an
`HttpApp`:

```ignite,fragment
type Bench {
  tools: Launcher
  entropy: Entropy
  tool: Setting
}

system Studio {
  resource ingress: TcpListener { address = "127.0.0.1", port = 8095 }
  resource tools: Launcher
  resource entropy: Entropy
  resource tool: Setting { flag = "--tool" }

  component server: HttpServer {
    listener = ingress
    application = HttpApp { state = Bench { tools = tools, entropy = entropy, tool = tool }, handle }
    executable = "studiod"
  }
}
```

Tests make capabilities with test-only constructors. Anywhere else these are E051.

| constructor | makes |
|---|---|
| `Launcher.new()` | a launcher with `maximum_children` 16 |
| `Entropy.new()` | an entropy source |
| `Setting.of(flag, value)` | a setting whose value is `value` |
| `CommandLine.of(arguments)` | a command line of `arguments` |
| `Environment.of(entries)` | an environment of `NAME=value` entries |

## Effects

Each method needs `<root>.<op>` on each root of its receiver. The ops differ per type, so
`requires [b.read]` on a record holding a `Directory` and a `Launcher` grants only the
directory's reads. `b.*` grants all. See [Effects](../effects.md).

| type | method | op |
|---|---|---|
| `Launcher` | `start`, `execute` | `launch` |
| `ChildProcess` | `wait` | `wait` |
| `ChildProcess` | `kill` | `kill` |
| `ChildProcess` | `id` | none |
| `Entropy` | `random_bytes` | `draw` |
| `Setting` | `value` | `argument` |
| `CommandLine` | `arguments` | `argument` |
| `Environment` | `get` | `variable` |

A `ChildProcess` that `start` returned has no roots, so the function that started it waits for
and kills it without further `requires`. A `ChildProcess` received as a parameter `p` needs
`p.wait` and `p.kill`.

## Child processes

```ignite,fragment
type Invocation {
  program: Text
  arguments: List<Text> = []
  directory: Text = ""
  environment: List<Text> = []
  inherit_environment: Bool = true
  timeout: Duration = 30 seconds
  output_limit: ByteSize = 1 MiB
  input: Bytes = b""
}

fn execute(launcher: Launcher, invocation: Invocation) -> Result<ChildExit, IoError>
fn start(launcher: Launcher, invocation: Invocation) -> Result<ChildProcess, IoError>
fn id(child: ChildProcess) -> Int
fn wait(child: ChildProcess) -> Result<ChildExit, IoError>
fn kill(child: ChildProcess) -> Result<Unit, IoError>
```

| method | what it does |
|---|---|
| `execute` | starts the child and waits for it on the calling task |
| `start` | starts the child and returns at once |
| `id` | the child's process id, which is also its process group id |
| `wait` | waits for the end; a second `wait` is the error `child already waited` |
| `kill` | sends SIGKILL to the child's group; `Ok` and nothing sent once the child was waited for |

| field | meaning |
|---|---|
| `program` | a bare name, looked up in the `PATH` of the child's environment, or an absolute path |
| `arguments` | passed as they are; no shell |
| `directory` | `""` keeps the program's own working directory; otherwise an absolute path, such as `Directory.absolute_path()` gives |
| `environment` | entries `NAME=value`, applied over the inherited environment, or over an empty one when `inherit_environment` is false |
| `timeout` | positive; on expiry the child's process group gets SIGKILL and the child ends `TimedOut` |
| `output_limit` | per stream, at most 16 MiB; output past it is read, discarded and counted in `dropped` |
| `input` | bytes written to the child's stdin, which is then closed; empty keeps stdin the null device |

`inherit_environment = true` hands the whole environment of the server, secrets included, to
every child. A caller that runs untrusted programs sets it to false and passes `PATH` and what
else the child needs.

Errors, each an `IoError` with this message:

| cause | message |
|---|---|
| a relative path with `/` | `program <p>: a relative path with /` |
| a program that does not exist | `program not found: <p>` |
| any other failure to start | `program <p>: <reason>` |
| a relative `directory` | `directory <d>: not absolute` |
| a `directory` that cannot be entered | `directory <d>: <reason>` |
| a `timeout` of zero or less | `timeout must be positive` |
| an `output_limit` over 16 MiB | `output_limit must be at most 16 MiB` |
| a launcher at its `maximum_children` | `too many children (<n>)` |

- A child counts against `maximum_children` from `start` until its `wait` returns. Nothing
  waits for a free slot.
- When the program ends in order, every live child's group gets SIGKILL before the process
  exits. When the program is killed, each direct child is killed too; its own children are not.
- A grandchild that left the child's group and holds a pipe open delays the result by at most
  100 ms.

## How a child ended

```ignite,fragment
type ChildExit {
  ending: Int
  code: Int
  stdout: Bytes
  stderr: Bytes
  dropped: Int
}

enum Ending {
  ExitCode(Int)
  BySignal(Int)
  TimedOut
  Killed
}

fn outcome(exit: ChildExit) -> Ending
fn succeeded(exit: ChildExit) -> Bool
```

| `ending` | meaning | `code` | `outcome` |
|---|---|---|---|
| 0 | exited | the exit status | `ExitCode(code)` |
| 1 | signalled | the signal number | `BySignal(code)` |
| 2 | timed out | -1 | `TimedOut` |
| 3 | killed | -1 | `Killed` |

`dropped` counts the bytes of both streams past `output_limit`. `succeeded` holds when the
child exited with status 0.

## Random bytes

```ignite,fragment
fn random_bytes(entropy: Entropy, count: Int) -> Result<Bytes, IoError>
```

`count` bytes from `/dev/urandom`, which the program opens once at launch. `count` is 0 to
65536, else the error `random_bytes: count must be in 0..=65536`.

## Settings

```ignite,fragment
fn value(setting: Setting) -> Text
```

The program reads each setting's flag from its arguments at launch, before it binds a
listener.

- The first `--flag V` or `--flag=V` wins. Arguments it does not know are ignored, so `--port`
  and `--ref` keep their meaning.
- A value after `--flag` that starts with `--` is no value. `--flag=V` takes `V` as it is.
- A required flag that is absent ends the program with exit status 2 and `error: <flag>:
  required argument missing`. A flag without a value gives exit status 2 and `error: <flag>
  needs a value`.
- A value that is not UTF-8 gives exit status 2 and `error: <flag>: value <bytes> is not UTF-8`.
- Flags are unique per system and differ from `--port` and `--ref` (E141).
- A `default` is part of the model `ignite plan` prints. A secret belongs in a flag given at
  launch, not in a `default`.

`ignite run` and `ignite apply` pass no setting values: a setting takes its `default`. A
`Setting` without `default` in a system they would start is E142.

## Command line

```ignite,fragment
fn arguments(command_line: CommandLine) -> List<Text>
```

Every argument after the executable's name, in order, flags included: `todo add "buy milk"
--port 0` gives `["add", "buy milk", "--port", "0"]`. The list is read once at launch. An
argument that is not UTF-8 ends the program before `main` with exit status 2 and `error:
argument <n>: value <bytes> is not UTF-8`. Parsing subcommands is left to the program.

## Environment

```ignite,fragment
fn get(environment: Environment, name: Text) -> Option<Text>
```

The value of the variable `name` when the program was launched, `None` when it was not set.
The environment is read once at launch; later changes are not seen. A variable whose name or
value is not UTF-8 is skipped. `Environment.of(entries)` splits each entry at its first `=`;
the last entry of a name wins, and an entry without `=` or with an empty name is skipped.

## Example

```ignite
import std.process.{Invocation, Ending, outcome}

test "a child's output and status are captured" {
  let exit = Launcher.new().execute(Invocation { program = "/bin/sh", arguments = ["-c", "echo out; exit 3"] })?
  assert exit.stdout == b"out\n"
  assert outcome(exit) == ExitCode(3)
  let echoed = Launcher.new().execute(Invocation { program = "/bin/cat", input = b"line\n" })?
  assert echoed.stdout == b"line\n"
}

test "a started child is killed" {
  let child = Launcher.new().start(Invocation { program = "/bin/sleep", arguments = ["30"] })?
  child.kill()?
  assert outcome(child.wait()?) == Killed
}

test "settings, command lines and environments made for a test" {
  assert Setting.of("--tool", "/bin/sh").value() == "/bin/sh"
  assert CommandLine.of(["add", "buy milk"]).arguments() == ["add", "buy milk"]
  let environment = Environment.of(["STORE=redis", "STORE=nats", "EMPTY="])
  assert environment.get("STORE") == Some("nats")
  assert environment.get("EMPTY") == Some("")
  assert environment.get("UNSET") == None
  assert Entropy.new().random_bytes(16)?.len() == 16
}
```
