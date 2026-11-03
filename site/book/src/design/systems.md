# Systems and assemblies

A `system` says what exists and how the parts relate; an `assembly` says where each part
runs. One system can be placed in different ways, and every placement is checked against the
same declaration. This page covers the system as a graph, the assembly kind `Deployment`,
how the compiler resolves an assembly, and what the design plans beyond that.

## A system is a graph

A system holds resources, components, contracts and policies. Each member has an identity,
`<System>.<name>`, and the compiler derives edges from how members name each other:

| edge | meaning | terminal example |
|---|---|---|
| `uses` | a bare member name in a property: the value is embedded | `Terminal.server` uses `Terminal.ingress` |
| `references` | a `Ref` value: the member is named, not embedded | `Terminal.promptProbe` references `Terminal.server` |
| `observes` | a `Signal` value | `Terminal.reachable` observes `Terminal.promptProbe.attempts` |
| `hosts` | from the assembly: one member runs inside another | `Terminal.server` hosts `Terminal.console` |
| `provisioning` | a resource and the component that binds it | `Terminal.ingress` to `Terminal.server` |
| `readiness` | what a probe or an evaluator waits for | `Terminal.promptProbe` to `Terminal.server` |
| `recovery` | a member and its supervisor machine | `Terminal.server` to `Terminal.server.supervisor` |

`ignite plan` prints these edges, `ignite explain` those of one member; a `uses` cycle is E105.

## An assembly binds members to places

`assembly name: K { … }` is a top-level declaration. Its kind in the standard library is
`Deployment { system: SystemRef, bindings: Map<Ref, Binding> }` from
[`std.deploy`](../reference/std/deploy.md), and `Binding` is an enum of four places:

| binding | built by | hosts | runs |
|---|---|---|---|
| `LocalSocket` | `localSocket()` | a `TcpListener` resource | nothing of its own |
| `NativeProcess(includes)` | `nativeProcess(includes = [..])` | an executable component, plus the members it embeds | its member, supervised |
| `WorkstationExecutable` | `workstationExecutable()` | any component | built, not started |
| `ScheduledProbeHost` | `scheduledProbeHost()` | a component with at least one output signal | one process per probe attempt |

The terminal's assembly, from the design file:

```ignite,fragment
assembly local: Deployment {
  system = Terminal
  bindings = {
    Terminal.ingress = localSocket()
    Terminal.server = nativeProcess(includes = [Terminal.console])
    Terminal.client = workstationExecutable()
    Terminal.promptProbe = scheduledProbeHost()
  }
}
```

Keys are qualified references (`Terminal.server`). `includes = [Terminal.console]` says the
console runs inside the server's process, as `application = console` already implies.

## A complete example

A listener, one server and an assembly that places both:

```ignite
import std.control.{Machine, Supervisor}
import std.policy.Restart
import std.deploy.{Deployment, localSocket, nativeProcess}

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

assembly local: Deployment {
  system = Hello
  bindings = {
    Hello.ingress = localSocket()
    Hello.server = nativeProcess(includes = [])
  }
}
```

`ignite explain` prints where each member goes:

```text
$ ignite explain hello.ignite local
local: assembly std.deploy.Deployment of Hello (hello.ignite:42:1)
  Hello.ingress = LocalSocket
  Hello.server = NativeProcess([]), executable hellod
```

## Resolution: the library decides, the compiler checks

The compiler does not know the variants of `Binding`. It knows three function names that a
binding type's module provides, and calls them during evaluation:

| function | question it answers |
|---|---|
| `admits(binding, member) -> Option<Text>` | may this member go here? `None`, or the reason it may not |
| `includes(binding) -> List<Ref>` | which members does this binding host besides its key? |
| `entries(binding) -> List<Text>` | which hooks must the member's kind declare for the binding to run it? |

`admits` receives a `std.deploy.Member`: the member's declaration, its kind, whether it is
executable and its unset outputs. With the answers, `ignite check` applies these rules to every
assembly, and `ignite plan` to the selected one:

| rule | diagnostic |
|---|---|
| every resource and component is a key or included by exactly one binding | [E110](../reference/diagnostics.md#e110) |
| no member is both a key and included, or included twice | [E111](../reference/diagnostics.md#e111) |
| `admits` refuses the member, or a contract or policy is used as a key | [E112](../reference/diagnostics.md#e112) |
| a binding's `includes` differs from what its member embeds | [E113](../reference/diagnostics.md#e113) |
| no assembly or system can be selected (several and no `--assembly`, or an unknown name) | [E114](../reference/diagnostics.md#e114) |
| a contract's signal has no producer the assembly runs | [E121](../reference/diagnostics.md#e121) |
| a run-time reference names a member the assembly does not run, or resolves to no listener or several | [E122](../reference/diagnostics.md#e122) |
| a hosted member's kind lacks a hook from `entries` (`main` and `machine` for `NativeProcess` and `ScheduledProbeHost`) | [E129](../reference/diagnostics.md#e129) |

Leave out the listener's binding and the example above fails:

```ignite,fails=E110
import std.control.{Machine, Supervisor}
import std.policy.Restart
import std.deploy.{Deployment, nativeProcess}

type Greeter { listener: TcpListener, executable: Text }

fn Greeter.main(self: Greeter) -> Result<Unit, IoError> = self.listener.accept()?.close()

fn Greeter.machine(self: Greeter, id: Text) -> Machine =
  Machine.Supervise(Supervisor { id = "{id}.supervisor", member = id, restart = Restart {} })

system Hello {
  resource ingress: TcpListener { address = "127.0.0.1", port = 7070 }
  component server: Greeter { listener = ingress, executable = "hellod" }
}

assembly local: Deployment {
  system = Hello
  bindings = { Hello.server = nativeProcess(includes = []) }
}
```

```text
error[E110]: `Hello.ingress` is neither a bindings key nor included by a binding of assembly `local`
```

The result is the model's `assembly.resolution`: per resource and component, its binding and
`via`, the key that hosts it; a kind without these functions gets `"resolution": null`.

## Who reads an assembly

| command | what it does with the assembly |
|---|---|
| `ignite check` | resolves every assembly and reports E110 to E114, E121, E122 and E129 |
| `ignite plan`, `ignite explain` | resolve the selected one and print it in the model |
| `ignite run` | starts what the resolution says runs, in the foreground ([guide](../guide/operate.md)) |
| `ignite apply` | applies a saved plan of the assembly through a local host ([guide](../guide/apply.md)) |
| `ignite build` | does not read assemblies; it builds every executable component |

## Composition and planning in the design

Section 5 of the design goes further. Components expose typed ports and require
capabilities; resources provide them; an assembly chooses implementations, placement,
isolation and packaging, and is checked against the same contracts and capability
requirements. A plan would then be feasible, infeasible (a concrete conflict between hard
constraints) or unresolved (facts that need admission, input or an analysis that does not
exist).

Typed ports, capability matching, preferences and these plan results are a design, not
implemented. What exists is the resolution above: placement checked by the binding type's
own rules. See the [roadmap](../roadmap.md).

Next: [Contracts, policies and control](contracts.md).
