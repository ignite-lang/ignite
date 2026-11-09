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
and explanations are all derived from it. There is one place to change and one place to
check.

## What one declaration says

The terminal reference system in
[`terminal.design.ignite`](https://github.com/ignite-lang/ignite/blob/main/examples/terminal/terminal.design.ignite)
declares a listener, a console, a Telnet server and a client. In the same language it
declares a probe, a contract and an assembly:

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
  good = prompt_received_before_timeout
  target = 99.9%
  window = rolling(30 days)
  missing_data = "insufficient_evidence"
}
```

`ignite build` turns the server into the executable `terminald` and the probe into
`promptProbe`. The client is declared as an `Application` without a `main`, so it is not
built. `ignite plan` evaluates every property and prints the system model. `ignite run`
starts the server, probes it every 10 seconds and evaluates `reachable` on every attempt.
The [terminal walkthrough](../guide/terminal.md) and
[plan, explain, run and replay](../guide/operate.md) show each step.

## Application and control

The design separates the application from its control. The controller creates and
supervises a Telnet server. The application decodes bytes and answers commands. Contract
evaluation observes the application. In the terminal system these are different pieces of
Ignite:

- the supervisor machine that `TelnetServer.machine` returns, which starts and restarts
  `terminald`;
- `TelnetServer.main` and the console's `respond` function, which serve the sessions;
- the evaluator that `Availability.machine` returns, which turns probe attempts into a
  standing.

The control side is a set of pure reducers around an effect boundary, with a journal of
every decision. [The run-time model](runtime.md) explains it.

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

The system surface is meant to be useful without the implementation subset. Most programs
compose imported components. Only module authors write the protocols and kinds underneath.

## How the pieces relate

| page | what it covers |
|---|---|
| [Intent and properties](intent.md) | a declaration is a typed record, quantities carry units, members have identities, and the vocabulary lives in the library |
| [Effects and capabilities](effects.md) | why authority is passed and never ambient, and why property expressions are pure |
| [Protocols and roles](protocols.md) | a protocol is library code, generic over any byte stream, and no general layer knows one |
| [Components](components.md) | a kind is a record type; `K.main` makes an executable; the hooks `validate` and `machine` |
| [Systems and assemblies](systems.md) | the system as a graph, and assemblies that bind each member to a place |
| [Contracts and policies](contracts.md) | indicators, objectives and agreements, policy authority, missing data |
| [The run-time model](runtime.md) | machines, the journal, adapters, replay, and the host that keeps them |
| [The compile-time evaluator](evaluator.md) | property evaluation with fuel, and the system model it produces |
| [One substrate, two targets](substrate.md) | the IO substrate written once, and the LLVM and Rust targets on top of it |

Read them in order: each page uses the terms of the ones before it. What the design reaches
for beyond the implementation is on the [roadmap](../roadmap.md).
