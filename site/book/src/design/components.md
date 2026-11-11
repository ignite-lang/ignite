# Components

A component is a unit of computation in a system: a server, a client, a console, a probe. Its
kind is an ordinary record type, so a component declaration is a record literal the compiler
checks field by field. A kind becomes an executable by declaring one function, `K.main`. It
takes part in `ignite check` and `ignite run` through two more, `validate` and `machine`.
This page explains why kinds are records and what each of the three functions is for.

## A kind is a record type

The terminal's server, from
[`examples/terminal/terminal.ignite`](https://github.com/ignite-lang/ignite/blob/main/examples/terminal/terminal.ignite):

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

The rules are those of record literals. The terminal leaves out `sessions`, `recovery` and
`rollout`, and gets their defaults. A field without a default that is left out is E104. A
field whose type is `Signal<T>` is an output: the component produces it, and setting it in a
declaration is E104 too. Property values are typed against the field, so
`maximum_line = 4096 bytes` is a `ByteSize`.

Because a kind is a type, a program can declare its own. Nothing in the compiler lists the
kinds of the standard library.

## `K.main` makes an executable

A kind produces an executable when its module declares `fn K.main(self: K) ->
Result<Unit, IoError>`. Its effects are inferred. Here is a kind of your own, with all three
functions:

```ignite
import std.control.{Machine, Supervisor}
import std.policy.Restart

type Greeter {
  listener: TcpListener
  greeting: Text = "hello"
  executable: Text
  recovery: Restart = Restart {}
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

fn Greeter.machine(self: Greeter, id: Text) -> Machine = Machine.Supervise(Supervisor {
  id = "{id}.supervisor"
  member = id
  restart = self.recovery
})

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

`ignite build` generates a `main` for each executable component, the same on both targets.
It binds each listener the component uses, builds the component's record value with the
resource handles filled in, and calls `K.main`. The
[command-line reference](../reference/cli.md#compiled-executables) lists what it does and
its exit statuses. `executable` names the program, by default the component's name.

The accept loop is Ignite code in the kind's module, not generated code. A failure inside a
spawned session ends that task and is reported with file, line and column. The server goes
on accepting ([Failures](../reference/tests.md#failures)).

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

`module("examples.terminal.commands")` fills a record of function types from the public
functions of the sibling file `commands.ignite`. The commands stay plain functions with
their own tests, and the declaration says which module implements the console.
[Module bindings](../reference/modules.md#module-bindings) has the rules.

## `validate` states the rules a type cannot express

A type can say that `greeting` is text. It cannot say that the text must not be empty. The
pure hook `validate` returns the reasons a value is refused, and `ignite check` reports each
reason as E120. Declare the `Greeter` above with `greeting = ""`, and the check refuses it:

```text
error[E120]: declaration `Hello.server` of kind `main.Greeter` is refused: greeting is empty
```

The compiler validates every member whose kind has the hook, and every record value inside a
property whose type has one. So `recovery = Restart { maximum_attempts = 0 }` is refused by
`Restart.validate`. The library states its own rules, in Ignite, and the compiler applies
them.

## `machine` gives the member its control

The pure hook `machine` returns the member's control machine for `ignite run`. The
`Greeter` returns a supervisor that restarts it under its `recovery` policy, as
`TelnetServer` does. `Machine` is a closed enum of four machines: supervisor, probe
scheduler, contract evaluator and rollout. A kind chooses and configures one. It cannot
define a new one, so every control decision stays in reducers the driver can journal and
replay. The evaluator calls the hook at plan time, so the initial machines are part of the
system model. [The run-time model](runtime.md) explains what the machines do.

The signatures of the hooks, and E109 for any other shape or an effect, are in
[System declarations](../reference/systems.md#hooks).

Next: [Systems and assemblies](systems.md).
