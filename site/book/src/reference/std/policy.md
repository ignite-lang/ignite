# std.policy

Recovery and rollout policies of a component. `Restart` says how a failed component is
restarted, `DrainRollout` how it is replaced by a new version. `ignite check` validates the
declarations, and `ignite run` applies them: the supervisor of [std.control](control.md)
restarts under `Restart`, and the rollout machine drains under `DrainRollout`.

- Layer: leaf.
- Source: [`std/policy.ignite`](https://github.com/ignite-lang/ignite/blob/main/std/policy.ignite).
- Import: `import std.policy.{Restart, DrainRollout, exponential, fixed}`.

## Backoff

```ignite,fragment
enum Backoff {
  Exponential(Duration, Duration),
  Fixed(Duration)
}

fn exponential(initial: Duration, maximum: Duration) -> Backoff
fn fixed(delay: Duration) -> Backoff
fn delay(backoff: Backoff, attempt: Int) -> Duration
```

| function | what it does |
|---|---|
| `exponential` | delays that double from `initial` and never exceed `maximum` |
| `fixed` | the same delay before every attempt |
| `delay` | the delay before retry `attempt`; the first retry is attempt 1 |

## Restart

```ignite,fragment
type Restart {
  backoff: Backoff = exponential(100 ms, 10 seconds)
  maximum_attempts: Int = 10
  interval: Duration = 5 minutes
  readiness_timeout: Duration = 10 seconds
}

fn allows(restart: Restart, attempt: Int) -> Bool
fn Restart.validate(self: Restart) -> List<Text>
```

Restart a failed component: at most `maximum_attempts` restarts within `interval`, each after
the backoff's delay. A process that reports no readiness within `readiness_timeout` of its
start is killed and counts as a failure.

- `allows` is whether restart number `attempt` is still allowed.
- `validate` gives the reasons `ignite check` reports as E120: `maximum_attempts` below 1, and
  an exponential backoff whose initial delay is greater than its maximum.

## DrainRollout

```ignite,fragment
type DrainRollout {
  deadline: Duration = 2 minutes
  on_deadline: Text = "disconnect_remaining"
}

enum OnDeadline {
  DisconnectRemaining
}

fn on_deadline(text: Text) -> Option<OnDeadline>
fn DrainRollout.validate(self: DrainRollout) -> List<Text>
```

Replace a component by draining its sessions: new sessions go to the new version, old ones
end by `deadline`, then `on_deadline` happens to the rest. `on_deadline` is written as text;
its closed set is `"disconnect_remaining"`, which ends the remaining sessions.

- `on_deadline` reads the text, `None` for anything else.
- `validate` refuses an unknown `on_deadline` and a deadline that is not positive (E120).

## In a declaration

From [`terminal.design.ignite`](https://github.com/ignite-lang/ignite/blob/main/examples/terminal/terminal.design.ignite),
inside the `TelnetServer` component:

```ignite,fragment
recovery = Restart {
  backoff = exponential(100 ms, 10 seconds)
  maximum_attempts = 10
  interval = 5 minutes
}

rollout = DrainRollout {
  deadline = 2 minutes
  on_deadline = "disconnect_remaining"
}
```

## Example

```ignite
import std.policy.*

test "an exponential backoff doubles up to its maximum" {
  let backoff = exponential(100 ms, 1 second)
  assert delay(backoff, 1) == 100 ms
  assert delay(backoff, 2) == 200 ms
  assert delay(backoff, 4) == 800 ms
  assert delay(backoff, 5) == 1 second
  assert delay(fixed(250 ms), 9) == 250 ms
}

test "Restart counts attempts and refuses impossible declarations" {
  let restart = Restart {}
  assert allows(restart, 10)
  assert !allows(restart, 11)
  assert Restart.validate(Restart {}) == []
  assert Restart.validate(Restart { maximum_attempts = 0 }) == ["maximum_attempts must be at least 1, found 0"]
}

test "DrainRollout reads its on_deadline" {
  assert DrainRollout {}.deadline == 2 minutes
  assert on_deadline("disconnect_remaining") == Some(DisconnectRemaining)
  assert DrainRollout.validate(DrainRollout { on_deadline = "keep" }) == ["unknown on_deadline \"keep\": the closed set is disconnect_remaining"]
}
```
