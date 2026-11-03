# Components

A component is a unit of computation in a system: a server, a client, a console, a probe. Its
kind is an ordinary record type, so a component declaration is a record literal the compiler
checks field by field. A kind becomes an executable by declaring one function, `K.main`, and
takes part in `ignite run` through two more, `machine` and `validate`. The running example is
[`examples/terminal/terminal.ignite`](https://github.com/ignite-lang/ignite/blob/main/examples/terminal/terminal.ignite).

## A kind is a record type

```ignite,fragment
component server: TelnetServer {
  listener = ingress
  application = console
  executable = "terminald"
}
```

`TelnetServer` is declared in `std.protocol.telnet` like any other record:

```ignite,fragment
type TelnetServer {
  listener: TcpListener
  application: CommandConsole
  executable: Text
  protocol: TelnetProfile = TelnetProfile {}
  sessions: SessionLimits = SessionLimits {}
  recovery: Restart = Restart {}
  rollout: DrainRollout = DrainRollout {}
}
```

The rules are those of record literals:

- a field with a default may be left out; the terminal leaves out `sessions`, `recovery` and
  `rollout`, and gets the defaults;
- a field without a default that is left out is E104;
- a field whose type is `Signal<T>` is an output: the component produces it, and setting it
  in a declaration is E104 too;
- property values are typed against the field: `maximum_line = 4096 bytes` is a `ByteSize`.

Because a kind is a type, a program can declare its own. Nothing in the compiler lists the
kinds of the standard library.

## `K.main` makes an executable

A kind produces an executable when its module declares `fn K.main(self: K) ->
Result<Unit, IoError>`. Its effects are inferred. A wrong signature is E109.

```ignite
type Greeter {
  listener: TcpListener
  greeting: Text = "hello"
  executable: Text
}

fn Greeter.main(self: Greeter) -> Result<Unit, IoError> {
  loop {
    let stream = self.listener.accept()?
    spawn greet(stream, self.greeting)
  }
}

fn Greeter.validate(self: Greeter) -> List<Text> = if self.greeting.is_empty() {
  ["greeting is empty"]
} else {
  []
}

fn greet(stream: TcpStream, greeting: Text) -> Result<Unit, IoError> requires [stream.write] {
  stream.write_all("{greeting}\r\n".to_bytes())?
  stream.close()
}

system Hello {
  resource ingress: TcpListener {
    address = "127.0.0.1"
    port = 7070
  }

  component server: Greeter {
    listener = ingress
    executable = "hellod"
  }
}
```

`ignite build` generates a `main` for each executable component, the same on both targets:

1. run under the substrate's process entry;
2. bind each `TcpListener` the component uses, honouring `--port` and printing the
   `listening on` line;
3. build the component's record value, with the resource handles and embedded components
   filled in;
4. call `K.main(value)` and map an `Err` to its exit code.

The accept loop is Ignite code in the kind's module, not generated code. A failure inside a
spawned session (a failed `assert`, an overflow, an index out of range) ends that task and is
reported with file, line and column; the server goes on accepting.

`executable` names the program, by default the component's name; the terminal sets `terminald`.

## Components without `main` are values

`CommandConsole` has no `main`, so `console` produces no executable. It is a value, embedded
where a bare name uses it: `application = console` passes the console's record to the
server, and `terminald` carries it.

```ignite,fragment
component console: CommandConsole {
  implementation = module("examples.terminal.commands")
  prompt = "terminal> "
  maximum_line = 4096 bytes
  command_schema = module("examples.terminal.command_schema")
}
```

## `module("…")` binds a sibling module

`implementation` has the type `Module<ConsoleCommands>`, where `ConsoleCommands` is a record
of function types:

```ignite,fragment
type ConsoleCommands {
  respond: fn(Text) -> Reply
}
```

`module("examples.terminal.commands")` names the sibling file `commands.ignite`. The compiler
fills the record's fields by name from that module's public functions, so `respond` becomes
`examples.terminal.commands.respond`. A module that is not found, or a function that is
missing or has another type, is E102. The commands stay plain functions with their own
tests, and the declaration says which module implements the console. A module path may also
name a std module: the design file's client uses `handler = module("std.protocol.telnet")`.

## The hooks `machine` and `validate`

Two more associated functions connect a kind to the tools. Both are pure, with a fixed
signature; any other shape, or an effect, is E109.

| hook | signature | used by |
|---|---|---|
| `validate` | `fn K.validate(self: K) -> List<Text>` | `ignite check`: each text is one E120 reason |
| `machine` | `fn K.machine(self: K, id: Text) -> std.control.Machine` | `ignite run`: the control machine of the member |

`validate` states the declaration rules a type cannot express. If the `Greeter` above were
declared with `greeting = ""`, `ignite check` would refuse it:

```ignite,fails=E120
type Greeter {
  greeting: Text = "hello"
}

fn Greeter.validate(self: Greeter) -> List<Text> = if self.greeting.is_empty() {
  ["greeting is empty"]
} else {
  []
}

system Hello {
  component server: Greeter { greeting = "" }
}
```

```text
error[E120]: declaration `Hello.server` of kind `main.Greeter` is refused: greeting is empty
```

The compiler validates every member whose kind has the hook, and every record value inside a
property whose type has one, so `recovery = Restart { maximum_attempts = 0 }` is refused by
`Restart.validate`.

`machine` returns the member's control machine. `TelnetServer` returns a supervisor that
restarts it under its `recovery` policy:

```ignite,fragment
fn TelnetServer.machine(self: TelnetServer, id: Text) -> Machine = Machine.Supervise(Supervisor {
  id = "{id}.supervisor"
  member = id
  restart = self.recovery
  rollout = Some(self.rollout)
})
```

`Machine` is a closed enum of four machines (`Supervise`, `Schedule`, `Evaluate`, `Roll`), so
a kind chooses and configures one; it cannot define a new one. The evaluator calls the hook
at plan time, so the initial machines are part of the system model.
[Contracts, policies and control](contracts.md) explains what each machine does.

## The terminal's components

| component | kind | `main` | `machine` | result |
|---|---|---|---|---|
| `console` | `CommandConsole` | no | no | a value inside `terminald` |
| `server` | `TelnetServer` | accept loop | supervisor | the executable `terminald` |
| `client` | `TelnetClient` | client session, exit 3 or 4 on failure | no | the executable `tn` |
| `promptProbe` (design file) | `TelnetPrompt` | one probe attempt | probe scheduler | the executable `promptProbe` |

Next: [Systems and assemblies](systems.md).
