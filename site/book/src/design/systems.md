# Systems and assemblies

A `system` says what exists and how the parts relate. An `assembly` says where each part
runs. One system can be placed in different ways, and every placement is checked against the
same declaration. This page explains the system as a graph, why placement is decided by the
library and checked by the compiler, and what the design plans beyond that.

## A system is a graph

A system holds resources, components, contracts and policies. Each member has an identity,
`<System>.<name>`, and the compiler derives edges from how members name each other. A member
name where a value is expected embeds the value: `Terminal.server` uses `Terminal.ingress`.
A name where a `Ref` is expected only names the member: the prompt probe references the
server it probes. A `Signal` value is observed: `Terminal.reachable` observes the probe's
attempts. The assembly and the control machines add edges of their own, such as the server
hosting the console.

The graph is what the tools reason over. `ignite plan` prints its edges, and `ignite
explain` prints those of one member. A cycle of uses among components is E105, while two
members that only reference each other are no cycle. The edge kinds are in
[System declarations](../reference/systems.md#edges).

## An assembly binds members to places

`assembly name: K { … }` is a top-level declaration. Its kind in the standard library is
`Deployment` from [`std.deploy`](../reference/std/deploy.md), a system and a binding for each
of its members. A binding is a place: a local socket for a listener, a native process for an
executable component, a workstation executable that is built but not started, and a probe
host that runs one process per probe attempt.

The `Greeter` of [Components](components.md#kmain-makes-an-executable) is placed by an
assembly of two bindings:

```ignite,fragment
import std.deploy.{Deployment, localSocket, nativeProcess}

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
local: assembly std.deploy.Deployment of Hello (hello.ignite:49:1)
  Hello.ingress = LocalSocket
  Hello.server = NativeProcess([]), executable hellod
```

The terminal's assembly also says `nativeProcess(includes = [Terminal.console])`: the
console runs inside the server's process, as `application = console` already implies. The
two must agree, so a declaration cannot claim a placement the code does not have.

## Resolution: the library decides, the compiler checks

The compiler does not know the variants of `Binding`. It knows three function names that a
binding type's module provides, and calls them during evaluation:

- `admits` answers whether a member may go to a binding, and if not, why;
- `includes` names the members a binding hosts besides its key;
- `entries` names the hooks a member's kind must declare for the binding to run it.

With the answers, the compiler checks that every resource and component has exactly one
place, that nothing is placed twice, that each place admits its member, and that every
contract's signal has a producer the assembly runs. Leave out the listener's binding above
and the check fails with [E110](../reference/diagnostics.md#e110). Every rule and its
diagnostic is in [System declarations](../reference/systems.md#assemblies), and the functions
are in [`std.deploy`](../reference/std/deploy.md#resolution-hooks).

So a new kind of place is a new binding type with its own `admits`, not a compiler change.

## Who reads an assembly

`ignite check` resolves every assembly. `ignite plan` and `ignite explain` resolve the
selected one and print it in the model. `ignite run` starts what the resolution says runs,
in the foreground. `ignite apply` hands a saved plan of the assembly to a local host.
`ignite build` does not read assemblies: it builds every executable component.
[The run-time model](runtime.md) explains the last two.

## Composition and planning in the design

Section 5 of the design goes further. Components expose typed ports and require
capabilities. Resources provide them. An assembly chooses implementations, placement,
isolation and packaging, and is checked against the same contracts and capability
requirements. A plan would then be feasible, infeasible or unresolved. Infeasible means a
concrete conflict between hard constraints. Unresolved means facts that need admission,
input or an analysis that does not exist.

Typed ports, capability matching, preferences and these plan results are designs, not
implemented. What exists is the resolution above: placement checked by the binding type's
own rules. See the [roadmap](../roadmap.md).

Next: [Contracts and policies](contracts.md).
