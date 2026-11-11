# Write a component kind

A component kind is a record type. Three associated functions connect it to the tools:
`main` makes an executable, `validate` adds declaration rules to `ignite check`, and `machine`
gives the member a control machine for `ignite run` and `ignite apply`. This page writes a
kind `Greeter` with all three and runs each one.
[Components](../design/components.md#kmain-makes-an-executable) explains the model, and
[System declarations](../reference/systems.md#hooks) lists the hooks' signatures.

## Declare the kind and its hooks

Create `greeter.ignite`:

```ignite
package greeter

import std.control.{Machine, Supervisor}
import std.policy.Restart
import std.deploy.{Deployment, localSocket, nativeProcess}

type Greeter {
  listener: TcpListener
  greeting: Text = "hello"
  recovery: Restart = Restart {}
  executable: Text
}

fn Greeter.main(self: Greeter) -> Result<Unit, IoError> {
  loop {
    let stream = self.listener.accept()?
    spawn greet(stream, self.greeting)
  }
}

fn greet(stream: TcpStream, greeting: Text) -> Result<Unit, IoError> requires [stream.write] {
  stream.write_all("{greeting}\r\n".to_bytes())?
  stream.close()
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

system Hello {
  resource ingress: TcpListener {
    address = "127.0.0.1"
    port = 7070
  }

  component server: Greeter {
    listener = ingress
    greeting = "hello from Ignite"
    executable = "hellod"
  }
}

assembly local: Deployment {
  system = Hello
  bindings = {
    Hello.ingress = localSocket()
    Hello.server = nativeProcess(includes = [])
  }
}
```

- `executable` names the program that `main` becomes.
- `main` is the accept loop. Each connection runs in its own task.
- `validate` returns one text per broken rule. It must be pure.
- `machine` picks one of the four machines of [`std.control`](../reference/std/control.md).
  `Supervise` restarts the process under the `recovery` policy.

A hook with another signature, or one with an effect, is
[E109](../reference/diagnostics.md#e109).

## Build and run it

```sh
ignite build greeter.ignite
target/ignite/greeter/bin/hellod --port 0
```

```text
built target/ignite/greeter/bin/hellod
listening on 127.0.0.1:35069
```

A client that connects to that port reads `hello from Ignite` and the connection closes.

## See `validate` refuse a declaration

Set the greeting to the empty text:

```ignite,fragment
component server: Greeter {
  listener = ingress
  greeting = ""
  executable = "hellod"
}
```

```sh
ignite check greeter.ignite
```

```text
greeter.ignite:44:3: error[E120]: declaration `Hello.server` of kind `greeter.Greeter` is refused: greeting is empty
```

`ignite check` exits 1. Each text that `validate` returns is one
[E120](../reference/diagnostics.md#e120).

## See the machine in the model

`ignite explain` shows the member with its defaults, its edges and the machine that
`machine` returned:

```sh
ignite explain greeter.ignite Hello.server
```

```text
Hello.server: component greeter.Greeter (greeter.ignite:44:3)
  listener = Hello.ingress
  greeting = "hello from Ignite"
  recovery = Restart { backoff: Exponential(100 ms, 10 seconds), maximum_attempts: 10, interval: 5 minutes, readiness_timeout: 10 seconds } (default)
  executable = "hellod"
recovery Hello.server.supervisor
uses Hello.ingress
provisions Hello.ingress
recovers Hello.server
resolution: NativeProcess([])
machine Hello.server.supervisor: Supervise
```

The assembly places the member in a native process:

```sh
ignite explain greeter.ignite local
```

```text
local: assembly std.deploy.Deployment of Hello (greeter.ignite:51:1)
  Hello.ingress = LocalSocket
  Hello.server = NativeProcess([]), executable hellod
```

`ignite run greeter.ignite` now builds `hellod`, starts it and restarts it when it exits
unasked. [Plan, explain, run and replay](../guide/operate.md) shows a run.
