# The idea

Ignite is an intent-oriented language for specifying, building, deploying and operating
systems. One program declares the components, the resources they use, the objectives they
must meet and the actions allowed to keep them met. The compiler checks that declaration as
a whole, turns it into native executables, and gives the same model to the tools that plan,
run and explain the system. This part of the book explains why the language looks the way it
does. The [reference](../reference/index.md) says exactly what it accepts.

## The problem

What a system is for is usually spread over many places: application code, infrastructure
definitions, deployment configuration, monitoring queries, runbooks and conventions. Those
copies drift apart. The design document names three ways this goes wrong:

- a declared latency target has no measurement behind it;
- a deployment policy ignores that it interrupts sessions;
- a provider replacement removes data that another part of the system assumes is durable.

Ignite's answer is one typed model. Executables, control behaviour, command line interfaces
and explanations are all derived from it, so there is one place to change and one place to
check.

## What one declaration says

The terminal reference system in
[`terminal.design.ignite`](https://github.com/ignite-lang/ignite/blob/main/examples/terminal/terminal.design.ignite)
declares a listener, a console, a Telnet server and a client, and then, in the same
language, a probe, a contract and an assembly:

```ignite,fragment
component server: TelnetServer {
  listener = ingress
  application = console
  executable = "terminald"
  recovery = Restart {
    backoff = exponential(100 ms, 10 seconds)
    maximum_attempts = 10
    interval = 5 minutes
  }
}

contract reachable: Availability {
  signal = promptProbe.attempts
  good = "prompt_received_before_timeout"
  target = 99.9%
  window = rolling(30 days)
  missing_data = "insufficient_evidence"
}
```

`ignite build` turns the server into the executable `terminald` and the probe into
`promptProbe`; the client is declared as an `Application` without a `main`, so it is not built.
`ignite plan` evaluates every property and prints the system model. `ignite run` starts the
server, probes it every 10 seconds and evaluates `reachable` on every attempt. The
[terminal walkthrough](../guide/terminal.md) and
[plan, explain, run and replay](../guide/operate.md) show each step.

## The execution model

The design describes a runtime that is a deterministic decision engine around an effect
boundary:

1. An ordered stream of events enters a reducer.
2. The reducer returns its next state and the effects it wants carried out.
3. A journal records each decision and each intent to act before anything happens.
4. Adapters carry out the effects and turn what they observe back into events.

What is built follows that shape. Under `ignite run`, the reducers are pure Ignite functions
in [`std.control`](../reference/std/control.md), stepped by the compile-time evaluator. The
driver journals every event before the step and every effect before dispatch. Its adapters
start processes, run probes and fire timers, and never decide anything. Because the
decisions are pure functions of the journaled events, `ignite replay` reproduces them from
the journal alone.

The design also separates the application from its control: the controller creates and
supervises a Telnet server, the application decodes bytes and answers commands, and
contract evaluation observes the application. In the terminal system these are different
pieces of Ignite:

- the supervisor machine that `TelnetServer.machine` returns, which starts and restarts
  `terminald`;
- `TelnetServer.main` and the console's `respond` function, which serve the sessions;
- the evaluator that `Availability.machine` returns, which turns probe attempts into a
  standing.

## Design rules

Section 3.1 of the design states the rules that every other part follows:

| rule | what it means in practice |
|---|---|
| One grammar for built-in and imported definitions | `TelnetServer`, `Restart` and `Availability` are ordinary types in `std/`, declared with the same syntax as yours |
| Domain nouns are library symbols | no keyword names a protocol, a policy or a deployment target; the compiler knows the shape of a declaration, not its vocabulary |
| Pure evaluation cannot perform ambient IO | property expressions and constants are pure; an effect there is a compile error |
| Runtime effects require declared capabilities | a function that reads a stream says so with `requires [stream.read]`, or the compiler infers it and its callers account for it |
| Generated objects keep their source | every member and property in the system model maps to a file, line and column |
| Compile-time evaluation is bounded | functions may recurse and loop, but evaluating a property stops at an explicit fuel limit with a diagnostic |

The system surface is meant to be useful without the implementation subset: most programs
compose imported components, and only module authors write the protocols and kinds
underneath.

## How the pieces relate

| page | what it covers |
|---|---|
| [Intent and properties](intent.md) | the declaration forms, typed quantities, identities and edges, and why domain nouns live in the library |
| [Effects and capabilities](effects.md) | `requires`, inference, effect roots, and why property expressions are pure |
| [Protocols and roles](protocols.md) | `protocol Telnet` and `protocol Http` written in Ignite, generic over any byte stream |
| [Components](components.md) | a kind is a record type; `K.main` makes an executable; the hooks `machine` and `validate` |
| [Systems and assemblies](systems.md) | the system as a graph, and assemblies that bind each member to a place |
| [Contracts, policies and control](contracts.md) | indicators, objectives and agreements, restart and rollout policies, standings |
| [One substrate, two targets](substrate.md) | the IO substrate written once in Rust, and the Rust and LLVM targets on top of it |
| [The compile-time evaluator](evaluator.md) | property evaluation with fuel, the system model, and the reducers it steps at run time |

Read them in order: each page uses the terms of the ones before it.

## What is not built

The design document reaches further than the implementation: a native controller compiled
from the same semantics, packages and lockfiles, more targets and a self-hosted compiler.
Those parts are designs, not features; the [roadmap](../roadmap.md) says where they stand.
These pages describe what the compiler in the repository does today and say so where they
quote the design.
