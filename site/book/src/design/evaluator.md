# The compile-time evaluator

A declaration is code: its properties are expressions that may call functions, loop and
build lists. Ignite runs that code at compile time, with an interpreter over the typed
program, and the result is the system model: every member with every property as a value.
The same interpreter steps the control machines when `ignite run` operates the system. This
page explains what the evaluator computes, its limits, the model it produces and how it is
used at run time. The commands are in [Plan, explain, run and replay](../guide/operate.md).

## What it is

The evaluator is the crate
[`crates/ignite-eval`](https://github.com/ignite-lang/ignite/tree/main/crates/ignite-eval).

| used by | for |
|---|---|
| `ignite check` | evaluating every property, running `validate` hooks, resolving every assembly |
| `ignite plan` | printing the system model |
| `ignite explain` | printing one system, member, property, assembly or machine |
| `ignite run`, `ignite replay` | stepping the `std.control` reducers |

No backend and no compiled program links it, and `ignite build` does not use it.

It evaluates the language a property expression may use: `let`, `var`, assignment, `if`,
`match`, the loops, closures applied during the evaluation, records, enums, lists, maps,
quantities, text and every builtin without an effect. Its semantics are those of the
targets: overflow is a failure, map order and quantity arithmetic are the same, `debug` gives
the same text.

## Properties are computed

```ignite
type Pool {
  workers: Int
  buffer: ByteSize
  names: List<Text>
}

fn workers_for(cores: Int) -> Int = cores * 2 + 1

fn names(count: Int) -> List<Text> {
  var out = []
  var i = 0
  while i < count {
    out.push("worker-{i}")
    i = i + 1
  }
  out
}

system Jobs {
  component pool: Pool {
    workers = workers_for(2)
    buffer = 16 KiB * 4
    names = names(3)
  }
}
```

```text
$ ignite explain jobs.ignite Jobs.pool
Jobs.pool: component main.Pool (jobs.ignite:20:3)
  workers = 5
  buffer = 64 KiB
  names = ["worker-0", "worker-1", "worker-2"]
```

A property expression must be pure, which is what makes this safe: an effectful call there
is [E116](../reference/diagnostics.md#e116) ([Effects and capabilities](effects.md)).

## Limits

Evaluation is bounded by fuel: one unit per expression evaluated, per loop iteration and per
call, 1,000,000 units per property expression. `--fuel N` on `check`, `plan` and `explain`
changes it. A value of more than 1,000,000 nodes stops the evaluation too.

| diagnostic | cause |
|---|---|
| [E115](../reference/diagnostics.md#e115) | the fuel or the value size ran out; names the limit and the function that was running |
| [E117](../reference/diagnostics.md#e117) | a failure: `assert`, overflow, an index, a `match`, `?` on `Err`; with the same report text the targets print |
| [E118](../reference/diagnostics.md#e118) | a result the model cannot encode: a closure, a function value that is not a named top-level function, a handle not produced by a member name |

```ignite,fails=E115
type Worker {
  steps: Int
}

fn forever(n: Int) -> Int {
  var i = n
  while i >= 0 {
    i = i + 1
  }
  i
}

system S {
  component w: Worker { steps = forever(0) }
}
```

```text
error[E115]: evaluating S.w.steps: the fuel limit of 1000000 units exceeded in main.forever
```

```ignite,fails=E117
type Worker {
  share: Int
}

fn share(total: Int, parts: Int) -> Int = total / parts

system S {
  component w: Worker { share = share(10, 0) }
}
```

```text
error[E117]: evaluating S.w.share: / failed at workers.ignite:5:43: 10 / 0
```

```ignite,fails=E118
type Worker {
  scale: fn(Int) -> Int
}

fn adder(n: Int) -> fn(Int) -> Int = |x| x + n

system S {
  component w: Worker { scale = adder(1) }
}
```

```text
error[E118]: evaluating S.w.scale: a closure cannot be encoded in the model
```

The evaluator and the two targets are held to one result: a fixture of at least 40 pure
functions prints the same `debug` text in the evaluator, a Rust build and an LLVM build.

## The system model

`ignite plan` prints the model of one system and at most one assembly as canonical JSON,
so equal inputs give equal bytes:

- object keys in byte order, two-space indentation, one key or item per line;
- members, edges and resolution entries sorted, so declaration order does not show;
- relative paths; no absolute path, timestamp, host name or compiler version.

| key | content |
|---|---|
| `formatVersion` | `"ignite-model/0.1"` |
| `systemId` | the system's name |
| `resources`, `components`, `contracts`, `policies` | every member, with its kind and evaluated properties |
| `edges` | `uses`, `references`, `observes`, `hosts`, `provisioning`, `readiness`, `recovery` |
| `assembly` | the selected assembly and its resolution, or `null` |
| `machines`, `observations` | the initial control machines; the signals, each with its producer and interval |
| `sourceMap` | every member and property path to `file:line:column` |

Values are encoded by type; a quantity keeps its unit, for example
`"buffer": {"bytes": 65536, "type": "ByteSize"}`. A change that removes or reinterprets a key
bumps `formatVersion`; an added key does not.

`ignite explain FILE PATH` renders the same model for one path in text: a system, a member,
a property, an assembly or a machine id such as `Terminal.server.rollout`. An unknown path is
[E119](../reference/diagnostics.md#e119).

## At run time: stepping the reducers

The control machines of `std.control` are Ignite functions, `step(machine, event) -> Step`,
and `ignite run` executes them in the evaluator rather than compiling them:

1. the driver keeps one ordered queue of events (a process started, a probe attempt done, a
   timer, a `rollout` command);
2. each event is journaled, then stepped through the machines it is routed to;
3. each effect a step returns (spawn, kill, set a timer, emit a signal) is journaled, then
   dispatched by an adapter;
4. adapters run on their own threads and feed results back as events.

Reducers never do IO and adapters never decide. A reducer that fails is
[E128](../reference/diagnostics.md#e128): that machine stops, the others go on, and the run
exits 1. `--fuel N` on `run` and `replay` bounds each reducer step; the plan inside the run
keeps the default fuel. The fixture `control_trace.ignite` steps the same reducers in the
evaluator and on both compiled targets, and the results agree.

## Replay

Because every decision is a pure step over journaled events, the journal is enough to
reproduce the run. `ignite replay` re-steps the journaled events and compares every effect,
signal, standing and consequence record with the journal, byte for byte. The first difference
is [E127](../reference/diagnostics.md#e127), with its sequence number. What a real process
contributes (a pid, a latency, an exit code) enters only through events, so replay is exact.

Next: [the roadmap](../roadmap.md) for what is designed and not built yet.
