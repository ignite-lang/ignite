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
  good = "prompt_received_before_timeout"
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
  (with the console inside it) to a native process, the probe to a scheduled probe host.

[Contracts, signals and policies](../design/contracts.md) explains the vocabulary.

## ignite plan

```sh
ignite plan examples/terminal/terminal.design.ignite
```

`plan` checks the program, evaluates every property expression with the compile-time
evaluator, resolves the assembly and prints the system model. The output is canonical: object
keys in byte order, two-space indentation, relative paths, no timestamp, host name or
compiler version. Equal inputs give equal bytes, so a model can be diffed and committed. The
top-level keys of the terminal's model:

```text
assembly  components  contracts  edges  formatVersion  machines
observations  policies  resources  sourceMap  systemId
```

`formatVersion` is `ignite-model/0.1`. `edges` is the dependency graph the compiler derived
from the declaration; for the design file it holds, among others:

```text
{"from":"Terminal.promptProbe","kind":"references","to":"Terminal.server"}
{"from":"Terminal.reachable","kind":"observes","to":"Terminal.promptProbe.attempts"}
{"from":"Terminal.server","kind":"hosts","to":"Terminal.console"}
{"from":"Terminal.server","kind":"recovery","to":"Terminal.server.supervisor"}
{"from":"Terminal.server","kind":"uses","to":"Terminal.ingress"}
```

`machines` lists the run-time machines the model implies: an evaluator per contract, a
scheduler for the probe, a supervisor and a rollout for the server.

| flag | meaning |
|---|---|
| `--assembly NAME` | the assembly to select (default: the program's only one) |
| `--out FILE` | write the model to a file instead of stdout |
| `--fuel N` | fuel per property expression (default 1,000,000 units) |
| `--for-apply --state DIR` | a saved plan for [local apply](apply.md) instead of the model |

## ignite explain

```sh
ignite explain examples/terminal/terminal.design.ignite Terminal
```

```text
Terminal: system (terminal.design.ignite:12:1)
  component Terminal.client: std.cli.Application
  component Terminal.console: std.console.CommandConsole
  contract Terminal.customerAccess: std.contract.SLA
  resource Terminal.ingress: TcpListener
  contract Terminal.promptFast: std.contract.ThresholdCompliance
  component Terminal.promptProbe: std.protocol.telnet.TelnetPrompt, executable promptProbe
  contract Terminal.reachable: std.contract.Availability
  component Terminal.server: std.protocol.telnet.TelnetServer, executable terminald
assembly local: std.deploy.Deployment
```

The path can name a system, a member, a property or an assembly. A contract shows its
evaluated properties, defaults marked, and what it observes:

```text
$ ignite explain examples/terminal/terminal.design.ignite Terminal.reachable
Terminal.reachable: contract std.contract.Availability (terminal.design.ignite:81:3)
  signal = Terminal.promptProbe.attempts
  good = "prompt_received_before_timeout"
  target = 99.9%
  window = Rolling(30 days)
  missing_data = "insufficient_evidence"
  minimum_completeness = 90% (default)
observes Terminal.promptProbe.attempts
readiness Terminal.promptProbe
machine Terminal.reachable.evaluator: Evaluate
standing: evaluated at run time (ignite run, ignite replay)
```

A property shows the value the evaluator computed, here `exponential(100 ms, 10 seconds)`
evaluated to a `Backoff` value and the default `readiness_timeout` filled in:

```text
$ ignite explain examples/terminal/terminal.design.ignite Terminal.server.recovery
Terminal.server.recovery: property std.policy.Restart (terminal.design.ignite:46:16)
  backoff = Exponential(100 ms, 10 seconds)
  maximum_attempts = 10
  interval = 5 minutes
  readiness_timeout = 10 seconds
```

An assembly shows where every member goes:

```text
$ ignite explain examples/terminal/terminal.design.ignite local
local: assembly std.deploy.Deployment of Terminal (terminal.design.ignite:108:1)
  Terminal.client = WorkstationExecutable
  Terminal.console via Terminal.server = NativeProcess([Terminal.console])
  Terminal.ingress = LocalSocket
  Terminal.promptProbe = ScheduledProbeHost, executable promptProbe
  Terminal.server = NativeProcess([Terminal.console]), executable terminald
```

## ignite run

`ignite run` is the supervisor process, in the foreground. It plans as `plan` does, builds
every buildable member, opens a journal, then starts the machines: the supervisor spawns
`terminald`, the probe scheduler starts probing once the server prints its `listening on`
line, and each contract evaluator publishes a standing on every probe result.

The run below uses the manual clock, so that time moves only when told to. It was driven
with four lines on stdin: `status`, `advance 30 seconds`, `status`, `stop`.

```sh
ignite run examples/terminal/terminal.design.ignite \
  --clock manual --port Terminal.ingress=0
```

Every journal record is one line on stderr. Shortened, the first probe attempt and its
evaluation read:

```text
8 0 ns event Terminal.server.supervisor: Start(20454 days)
9 0 ns effect Terminal.server.supervisor: Spawn("Terminal.server", 1)
14 0 ns event Terminal.server.supervisor: Ready(1769499)
18 0 ns effect Terminal.promptProbe.scheduler: Attempt("Terminal.promptProbe", 1)
21 0 ns event Terminal.promptProbe.scheduler: AttemptDone(1, { latency: 140315 ns, received: true })
24 0 ns signal Terminal.promptProbe.scheduler: { ref: "Terminal.promptProbe.attempts", value: { latency: 140315 ns, received: true } }
33 0 ns standing Terminal.reachable.evaluator: { …, executed: 1, expected: 1, good: 1, status: Compliant, … }
34 10 seconds event Terminal.promptProbe.scheduler: Timer(0)
35 10 seconds effect Terminal.promptProbe.scheduler: Attempt("Terminal.promptProbe", 2)
```

and the stop at 30 seconds:

```text
85 30 seconds event Terminal.server.rollout: Control("stop")
87 30 seconds effect Terminal.server.supervisor: Kill("Terminal.server", 1769499, 15)
88 30 seconds event Terminal.server.supervisor: Exited(1769499, 0)
91 30 seconds segment: { clock: "manual", declared: ["Terminal.client"], generation: 1, start: 0 ns, stop: Some(30 seconds) }
```

The second `status` printed the standings on stdout, after four probe attempts:

```text
standing Terminal.reachable: Standing { contract: "Terminal.reachable", status: Status.Compliant, good: 4, executed: 4, expected: 4, completeness: 100%, from: 0 ns, to: 30 seconds, coverage: 0.0011%, calculation: "ignite-sli/0.1", version: 84 }
port Terminal.ingress: 33517
declared, not buildable in slice 12: Terminal.client
```

The first `status`, before any attempt, printed `status: Status.InsufficientEvidence` with
`executed: 0`: zero traffic does not prove compliance. `coverage` is the share of the 30-day
window that the evidence covers. `Terminal.client` is an `Application` without a `main`, so
the run lists it as declared but does not build it.

How the run works:

- Each event is written to the journal, then stepped through pure reducers from
  [`std.control`](../reference/std/control.md); each effect a reducer returns is written to
  the journal, then carried out. Reducers never do IO; the driver's adapters never decide.
- `Restart` is the supervisor's policy: an exit nobody asked for is a failure, retried with
  the declared backoff until `maximum_attempts` within `interval`, then the machine gives up.
- `rollout Terminal.server` on stdin builds the next generation and replaces the server
  under `DrainRollout`: the old process closes its listener and drains, the new one binds.
  The model declares the disruption (connections refused between listener close and
  rebind) rather than claiming zero downtime.
- `stop`, SIGINT or SIGTERM drain every process with SIGTERM, wait up to the member's drain
  deadline, kill what is left and exit.

| stdin line | effect |
|---|---|
| `status` | the standing of every contract, the state of every machine, the ports in use |
| `rollout <member>` | start the member's rollout machine |
| `stop` | stop the run, as SIGINT does |
| `advance <quantity>` | manual clock only: move time, for example `advance 2 seconds` |

| flag | meaning |
|---|---|
| `--state DIR` | journal, lock and executables (default `target/ignite-run/<assembly>/`) |
| `--clock real\|manual` | the clock the reducers see (default `real`) |
| `--epoch RFC3339`, `--settle DURATION` | the manual clock's epoch (default `2026-01-01T00:00:00Z`) and settle time (default `2 s`) |
| `--port RESOURCE=PORT` | bind a resource to a port; `0` binds a free one |
| `--target rust\|llvm`, `--release` | how the members are built |
| `--json` | print the journal records on stdout |
| `--fresh` | rename an existing journal and start a new one |

`ignite run` exits 0 on a requested stop, 1 when a machine gave up or failed, and 2 on a
diagnostic before the start.

## The journal

The state directory holds the run's record:

```text
target/ignite-run/local/
  bin/            the executables, one directory per generation
  build/          the generated build project
  journal.jsonl   one JSON record per line
  lock            held while a run or host owns the directory
```

The first line of `journal.jsonl` is a header with the full model. Every other record has a
`seq` (gap-free from 0), an `at` on the run's clock, a `kind` (`segment`, `event`, `effect`,
`signal`, `standing`, `consequence`, `error`) and a value. Effects are synced to disk before
they are carried out. A second run on the same state directory with the same model appends a
new segment.

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
events through the same reducers, compares every effect and standing it gets with the one
on record (E127), and prints the final standings. Because the reducers are pure Ignite
functions, a run's decisions can be reproduced from its journal alone.

Next: [local apply, host and inspect](apply.md).
