# The compile-time evaluator

A declaration is code: its properties are expressions that may call functions, loop and
build lists. Ignite runs that code at compile time, with an interpreter over the typed
program, and the result is the system model: every member with every property as a value.
The same interpreter steps the control machines when `ignite run` operates the system. This
page explains what the evaluator computes, why it is bounded, and what the model is for.
The commands are in [Plan, explain, run and replay](../guide/operate.md).

## What it is

The evaluator is part of the compiler, not of a compiled program. `ignite check` uses it to
evaluate every property, run the `validate` hooks and resolve every assembly. `ignite plan`
and `ignite explain` use it to print the model. `ignite run` and `ignite replay` use it to
step the control machines ([The run-time model](runtime.md)). `ignite build` does not use
it.

It evaluates the language a property expression may use: `let`, `var`, assignment, `if`,
`match`, the loops, closures applied during the evaluation, records, enums, lists, maps,
quantities, text and every builtin without an effect. Its semantics are those of the
targets. Overflow is a failure, map order and quantity arithmetic are the same, and `debug`
gives the same text. A test in the repository holds the evaluator and both targets to the
same result over a set of pure functions.

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

A property may loop, so the compiler must be able to stop. Evaluation is bounded by fuel:
one unit per expression evaluated, per loop iteration and per call, 1,000,000 units per
property expression. The size of a value and the depth of nested calls are bounded too.
Running out of any of them is [E115](../reference/diagnostics.md#e115), which names the limit
and the function that was running. `--fuel N` on `plan` and `explain` changes the fuel
([command line](../reference/cli.md#ignite-plan)).

A failure while evaluating, such as a failed `assert`, an overflow or `?` on `Err`, is
[E117](../reference/diagnostics.md#e117), with the report text the targets print. A result
the model cannot hold, such as a closure, is [E118](../reference/diagnostics.md#e118).

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

## The system model

The result of the evaluation is the system model: every member with its kind and its
evaluated properties, the edges of the graph, the selected assembly and its resolution, the
initial control machines, and a source map from every property to its file, line and
column. `ignite plan` prints it as canonical JSON, so equal inputs give equal bytes, and
declaration order, absolute paths and timestamps do not show. Two plans of one program can
be compared with `diff`, and a tool can read the model without parsing Ignite.

`ignite explain FILE PATH` renders the same model for one path in text: a system, a member,
a property, an assembly, or a machine such as `Terminal.server.rollout`. The keys and the
encoding are in [System declarations](../reference/systems.md#the-system-model).

The model is also the contract between the compiler and the run-time driver: the journal of
a run starts with it, and `ignite replay` refuses a journal whose model differs from the
current plan ([The run-time model](runtime.md#replay)).

Next: [One substrate, two targets](substrate.md).
