# Write a probe and a contract over it

A probe is a component kind whose executable makes one attempt against a member and prints
its outcome as one line. `ignite run` schedules the attempts, and a contract decides for each
outcome whether it was good. This page follows the HTTP probe of the todo example,
[`examples/todo/health.ignite`](https://github.com/ignite-lang/ignite/blob/main/examples/todo/health.ignite).
The idea behind probes and contracts is in
[Contracts, policies and control](../design/contracts.md).

## 1. The outcome type and its predicate

One attempt's outcome is an ordinary record. A predicate over it says what "good" means:

```ignite,fragment
type HealthCheck {
  answered: Bool
  status: Int
  latency: Duration
}

fn healthy(check: HealthCheck) -> Bool = check.answered && check.status == 200
```

The outcome may not hold a fn, a handle, a reference or a signal ([E151](../reference/diagnostics.md#e151)).

## 2. The kind

```ignite,fragment
type HttpHealth {
  target: Ref
  path: Text = "/health"
  interval: Duration = 10 seconds
  timeout: Duration = 2 seconds
  checks: Signal<HealthCheck>
  executable: Text
}
```

- `target: Ref` names the member to probe without embedding it. `target = server` in the
  system becomes a reference to `TodoService.server`.
- `checks: Signal<HealthCheck>` is the signal the attempts produce. A contract names it as
  `health.checks`.
- `interval` and `timeout` drive the scheduler. The timeout must be below the interval
  ([E123](../reference/diagnostics.md#e123)).

## 3. `main`: one attempt

`main` resolves the target, makes the attempt and prints the outcome in the model encoding,
then exits:

```ignite,fragment
fn HttpHealth.main(self: HttpHealth) -> IoResult<Unit> {
  let started = Clock.monotonic()
  let endpoint = Endpoint.of(self.target)?
  let status = match TcpStream.connect(endpoint.address, endpoint.port) {
    Err(_) => 0
    Ok(stream) => {
      stream.set_idle_timeout(self.timeout.to_ms())?
      let found = match ask(stream, request_text(self.path, endpoint.address)) {
        Ok(line) => status_of(line)
        Err(_) => 0
      }
      _ = stream.close()
      found
    }
  }
  let check = HealthCheck { answered = status > 0, status, latency = Clock.monotonic() - started }
  Stdout.write_all(encode_check(check).to_bytes())
}
```

- `Endpoint.of(self.target)` reads the `--ref` binding that `ignite run` passes. Without one
  it fails, and the probe exits 1.
- A failed attempt is an outcome, not an error: a refused connection prints
  `answered = false, status = 0` and exits 0.
- `encode_check` writes the line by hand, keys in byte order. The driver decodes it with the
  element type of `checks`.

Run by hand against a running `todod`:

```sh
target/ignite/todo/bin/health --ref TodoService.server=127.0.0.1:8080
```

```text
{"fields":{"answered":true,"latency":{"ns":318153,"type":"Duration"},"status":200},"type":"examples.todo.health.HealthCheck"}
```

## 4. `machine` and `validate`

`machine` returns the scheduler `ignite run` steps: one attempt per `interval` from the
target's first readiness, each with a deadline of `timeout`. `validate` returns the reasons a
declaration is refused:

```ignite,fragment
fn HttpHealth.machine(self: HttpHealth, id: Text) -> Machine = Machine.Schedule(ProbeScheduler {
  id = "{id}.scheduler"
  member = id
  target = self.target.to_text()
  signal = self.checks.to_text()
  interval = self.interval
  timeout = self.timeout
})

fn HttpHealth.validate(self: HttpHealth) -> List<Text> = path_reasons(self.path)
```

`path_reasons` refuses a path without a leading `/` or with a space. With `path = "health"`,
`ignite check` prints:

```text
examples/todo/todo.ignite:319:3: error[E120]: declaration `TodoService.health` of kind `examples.todo.health.HttpHealth` is refused: path must start with /, found health
```

The hooks' signatures are in [System declarations](../reference/systems.md#hooks).

## 5. The component and the contract

```ignite,fragment
component health: HttpHealth {
  target = server
  path = "/health"
  interval = 10 seconds
  timeout = 2 seconds
}

contract reachable: Availability {
  signal = health.checks
  good = healthy
  target = 99%
  window = rolling(5 minutes)
  missing_data = "insufficient_evidence"
}
```

`Availability` is generic over the outcome type, so `good` must be a `fn(HealthCheck) ->
Bool`. A predicate over another type is refused:

```text
examples/todo/todo.ignite:328:12: error[E040]: type mismatch: expected `fn(HealthCheck) -> Bool`, found `fn(ProbeAttempt) -> Bool`
```

The assembly places the probe with `scheduledProbeHost()`:

```ignite,fragment
TodoService.health = scheduledProbeHost()
```

## 6. Check it

```sh
ignite explain examples/todo/todo.ignite TodoService.reachable
```

```text
TodoService.reachable: contract std.contract.Availability<examples.todo.health.HealthCheck> (todo.ignite:326:3)
  signal = TodoService.health.checks
  good = examples.todo.health.healthy
  target = 99%
  window = Rolling(5 minutes)
  missing_data = "insufficient_evidence"
  minimum_completeness = 90% (default)
observes TodoService.health.checks
readiness TodoService.health
machine TodoService.reachable.evaluator: Evaluate
standing: evaluated at run time (ignite run, ignite replay)
```

Under `ignite run` each attempt's line arrives as an `AttemptDone` event, and the evaluator
records whether `healthy` held:

```text
18 0 ns event TodoService.health.scheduler: AttemptDone(1, "{\"fields\":{\"answered\":true,\"latency\":{\"ns\":300232,\"type\":\"Duration\"},\"status\":200},\"type\":\"examples.todo.health.HealthCheck\"}")
22 0 ns event TodoService.reachable.evaluator: Emitted("TodoService.health.checks", { good: true, slot: 0 ns })
24 0 ns standing TodoService.reachable.evaluator: { calculation: "ignite-sli/0.1", completeness: 100%, contract: "TodoService.reachable", coverage: 0%, executed: 1, expected: 1, from: 0 ns, good: 1, status: Compliant, to: 0 ns, version: 24 }
```

The contract kinds and their properties are in [`std.contract`](../reference/std/contract.md),
the windows and missing-data rules in [`std.sli`](../reference/std/sli.md).
