# Plan, explain, run and replay

`terminal.ignite` declares what the terminal is made of. The full declaration,
[`terminal.design.ignite`](https://github.com/ignite-lang/ignite/blob/main/examples/terminal/terminal.design.ignite),
also says what it promises and how it is kept that way: a probe, three contracts, a restart
policy, a drain rollout and an assembly that places every member. Four commands work on it:

| command | what it does |
|---|---|
| `ignite plan` | evaluates every property at compile time and prints the system model as canonical JSON |
| `ignite explain` | prints one system, member, property or assembly of that model for people |
| `ignite run` | builds the members, starts them, probes them and evaluates the contracts, in the foreground |
| `ignite replay` | re-steps a run's journal and checks that every decision comes out the same |

## The declaration

The parts that `terminal.ignite` does not have:

```ignite,fragment
component promptProbe: TelnetPrompt {
  target = server
  interval = 10 seconds
  timeout = 2 seconds
  expected_prompt = "terminal> "
  close_after_probe = true
}

contract reachable: Availability {
  signal = promptProbe.attempts
  good = prompt_received_before_timeout
  target = 99.9%
  window = rolling(30 days)
  missing_data = "insufficient_evidence"
}

assembly local: Deployment {
  system = Terminal
  bindings = {
    Terminal.ingress = localSocket()
    Terminal.server = nativeProcess(
      includes = [Terminal.console]
    )
    Terminal.client = workstationExecutable()
    Terminal.promptProbe = scheduledProbeHost()
  }
}
```

- `promptProbe` connects to the server every 10 seconds and waits at most 2 seconds for the
  prompt. Each attempt is a signal, `promptProbe.attempts`.
- `reachable` is a contract over that signal: 99.9 % of attempts in a rolling 30-day window
  must see the prompt. Missing attempts count as missing evidence, not as success.
- The server also carries `recovery = Restart { … }` and `rollout = DrainRollout { … }`.
- `assembly local` binds every member to a place: the listener to a local socket, the server
  with the console inside it to a native process, the probe to a scheduled probe host.

[Contracts, policies and control](../design/contracts.md) explains the vocabulary.

## ignite plan

```sh
ignite plan examples/terminal/terminal.design.ignite --out model.json
```

`plan` checks the program, evaluates every property expression with the compile-time
evaluator, resolves the assembly and writes the system model. Without `--out` it prints the
model on stdout. Equal inputs give equal bytes, so a model can be diffed and committed. The
model's top-level keys:

```text
assembly  components  contracts  edges  formatVersion  machines
observations  policies  resources  sourceMap  systemId
```

`edges` is the dependency graph the compiler derived from the declaration. Among others:

```text
{"from":"Terminal.promptProbe","kind":"references","to":"Terminal.server"}
{"from":"Terminal.reachable","kind":"observes","to":"Terminal.promptProbe.attempts"}
{"from":"Terminal.server","kind":"hosts","to":"Terminal.console"}
{"from":"Terminal.server","kind":"recovery","to":"Terminal.server.supervisor"}
{"from":"Terminal.server","kind":"uses","to":"Terminal.ingress"}
```

`machines` lists the run-time machines the model implies: an evaluator per contract, a
scheduler for the probe, a supervisor and a rollout for the server.

## ignite explain

```sh
ignite explain examples/terminal/terminal.design.ignite Terminal
```

```text
Terminal: system (terminal.design.ignite:13:1)
  component Terminal.client: std.cli.Application
  component Terminal.console: std.console.CommandConsole
  contract Terminal.customerAccess: std.contract.SLA<std.sli.ProbeAttempt>
  resource Terminal.ingress: TcpListener
  contract Terminal.promptFast: std.contract.ThresholdCompliance<std.sli.ProbeAttempt>
  component Terminal.promptProbe: std.protocol.telnet.TelnetPrompt, executable promptProbe
  contract Terminal.reachable: std.contract.Availability<std.sli.ProbeAttempt>
  component Terminal.server: std.protocol.telnet.TelnetServer, executable terminald
assembly local: std.deploy.Deployment
```

The path can name a system, a member, a property or an assembly. A contract shows its
evaluated properties, defaults marked, and what it observes:

```text
$ ignite explain examples/terminal/terminal.design.ignite Terminal.reachable
Terminal.reachable: contract std.contract.Availability<std.sli.ProbeAttempt> (terminal.design.ignite:82:3)
  signal = Terminal.promptProbe.attempts
  good = std.sli.prompt_received_before_timeout
  target = 99.9%
  window = Rolling(30 days)
  missing_data = "insufficient_evidence"
  minimum_completeness = 90% (default)
observes Terminal.promptProbe.attempts
readiness Terminal.promptProbe
machine Terminal.reachable.evaluator: Evaluate
standing: evaluated at run time (ignite run, ignite replay)
```

A property shows the value the evaluator computed. Here `exponential(100 ms, 10 seconds)`
became a `Backoff` value, and the default `readiness_timeout` is filled in:

```text
$ ignite explain examples/terminal/terminal.design.ignite Terminal.server.recovery
Terminal.server.recovery: property std.policy.Restart (terminal.design.ignite:47:16)
  backoff = Exponential(100 ms, 10 seconds)
  maximum_attempts = 10
  interval = 5 minutes
  readiness_timeout = 10 seconds
```

An assembly shows where every member goes:

```text
$ ignite explain examples/terminal/terminal.design.ignite local
local: assembly std.deploy.Deployment of Terminal (terminal.design.ignite:109:1)
  Terminal.client = WorkstationExecutable
  Terminal.console via Terminal.server = NativeProcess([Terminal.console])
  Terminal.ingress = LocalSocket
  Terminal.promptProbe = ScheduledProbeHost, executable promptProbe
  Terminal.server = NativeProcess([Terminal.console]), executable terminald
```

## ignite run

`ignite run` plans as `plan` does, builds every buildable member, opens a journal and starts
the machines. The supervisor spawns `terminald`. The probe scheduler starts probing once the
server prints its `listening on` line. Each contract evaluator publishes a standing on every
probe result.

The run below uses the manual clock, so time moves only when told to. `--port
Terminal.ingress=0` binds a free port. It was driven with four lines on stdin: `status`,
`advance 30 seconds`, `status`, `stop`.

```sh
ignite run examples/terminal/terminal.design.ignite \
  --clock manual --port Terminal.ingress=0
```

Every journal record is one line on stderr. Shortened, the start and the first probe attempt
read:

```text
9 0 ns effect Terminal.server.supervisor: Spawn("Terminal.server", 1)
14 0 ns event Terminal.server.supervisor: Ready(1993862)
18 0 ns effect Terminal.promptProbe.scheduler: Attempt("Terminal.promptProbe", 1)
21 0 ns event Terminal.promptProbe.scheduler: AttemptDone(1, "{\"fields\":{\"latency\":{\"ns\":200107,\"type\":\"Duration\"},\"received\":true},\"type\":\"std.sli.ProbeAttempt\"}")
33 0 ns standing Terminal.reachable.evaluator: { calculation: "ignite-sli/0.1", completeness: 100%, contract: "Terminal.reachable", coverage: 0%, executed: 1, expected: 1, from: 0 ns, good: 1, status: Compliant, to: 0 ns, version: 33 }
34 10 seconds event Terminal.promptProbe.scheduler: Timer(0)
35 10 seconds effect Terminal.promptProbe.scheduler: Attempt("Terminal.promptProbe", 2)
```

and the stop at 30 seconds:

```text
85 30 seconds event Terminal.server.rollout: Control("stop")
87 30 seconds effect Terminal.server.supervisor: Kill("Terminal.server", 1993862, 15)
88 30 seconds event Terminal.server.supervisor: Exited(1993862, 0)
91 30 seconds segment: { clock: "manual", declared: ["Terminal.client"], generation: 1, start: 0 ns, stop: Some(30 seconds) }
```

The second `status` on stdout, after four probe attempts, shortened to the reachability
standing and the port:

```text
standing Terminal.reachable: Standing { contract: "Terminal.reachable", status: Status.Compliant, good: 4, executed: 4, expected: 4, completeness: 100%, from: 0 ns, to: 30 seconds, coverage: 0.0011%, calculation: "ignite-sli/0.1", version: 84 }
port Terminal.ingress: 44687
```

With `missing_data = "insufficient_evidence"`, a window without an executed attempt has the
status `Status.InsufficientEvidence`: zero traffic does not prove compliance. `coverage` is the
share of the 30-day window that the evidence covers. `Terminal.client` is an `Application`
without a `main`, so the run lists it as declared and does not build it.

Each event is written to the journal, then stepped through pure reducers, and each effect a
reducer returns is written before it is carried out. [The run-time model](../design/runtime.md#the-decision-loop)
explains the decision loop. `rollout Terminal.server` on stdin replaces the server:
[Replace a server with a rollout](../howto/rollout.md). Every flag and stdin command is in the
[command-line reference](../reference/cli.md#ignite-run).

## The state directory

The run's record is in its state directory, by default `target/ignite-run/<assembly>/`:

```text
target/ignite-run/local/
  bin/            the executables, one directory per generation
  build/          the generated build project
  journal.jsonl   one JSON record per line
  lock            held while a run or host owns the directory
```

A second run on the same state directory with the same model appends a new segment to the
journal. [System declarations](../reference/systems.md#the-journal) documents the journal's records.

## ignite replay

```sh
ignite replay examples/terminal/terminal.design.ignite --state target/ignite-run/local
```

```text
Terminal.customerAccess Standing { contract: "Terminal.customerAccess", status: Status.Compliant, good: 4, executed: 4, expected: 4, completeness: 100%, from: 0 ns, to: 30 seconds, coverage: 0.0011%, calculation: "ignite-sli/0.1", version: 78 }
Terminal.promptFast Standing { contract: "Terminal.promptFast", status: Status.Compliant, good: 4, executed: 4, expected: 4, completeness: 100%, from: 0 ns, to: 30 seconds, coverage: 0.8333%, calculation: "ignite-sli/0.1", version: 81 }
Terminal.reachable Standing { contract: "Terminal.reachable", status: Status.Compliant, good: 4, executed: 4, expected: 4, completeness: 100%, from: 0 ns, to: 30 seconds, coverage: 0.0011%, calculation: "ignite-sli/0.1", version: 84 }
```

`replay` checks the journal's header against the current plan (E126), feeds the journaled
events through the same reducers, compares every effect and standing with the one on record
(E127), and prints the final standings. A run's decisions can be reproduced from its journal
alone.

Next: [host and apply](apply.md).
