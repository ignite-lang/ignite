# Contracts, policies and control

A contract says what the system promises; a policy says what may be done to keep the
promise. Both are declarations, checked at compile time and evaluated at run time by pure
control machines that `ignite run` steps. This page explains the ideas behind contracts and
policies, the kinds the standard library ships, and how a probe's attempts become a standing.
It uses
[`examples/terminal/terminal.design.ignite`](https://github.com/ignite-lang/ignite/blob/main/examples/terminal/terminal.design.ignite);
the commands are in [Plan, explain, run and replay](../guide/operate.md).

## Indicators, objectives and agreements

Design section 4.1 separates three layers:

| layer | adds |
|---|---|
| indicator (SLI) | an eligible population, the good outcomes, the measurement source, the accounting rules |
| objective (SLO) | a target and a window |
| agreement (SLA) | contractual scope, period, exclusions, reporting and consequences |

A standing is `compliant`, `violated` or `insufficient_evidence`, and it carries its
interval, completeness and calculation version. Two rules from the design shape everything
below: "Zero traffic does not automatically prove compliance. Missing telemetry does not
become a successful sample."

## Policy authority

Section 4.2: "Objectives report whether intent is met. Policies decide what actions are
permitted in response." A contract never restarts anything. A policy such as `Restart` bounds
what the supervisor may do, and the supervisor acts only inside those bounds. An SLA
consequence can emit a typed breach event; the design states that Ignite does not infer
payment obligations or execute settlement.

## The kinds

From [`std.contract`](../reference/std/contract.md) and
[`std.policy`](../reference/std/policy.md):

| kind | fields (defaults in brackets) |
|---|---|
| `Availability` | `signal`, `good`, `target`, `window`, `missing_data` (`"insufficient_evidence"`), `minimum_completeness` (90%) |
| `ThresholdCompliance` | as `Availability`, plus `threshold: Duration` |
| `SLA` | `signal`, `good`, `target`, `period`, `exclusions` (`[]`), `consequence`, `minimum_completeness` (90%) |
| `Restart` | `backoff` (`exponential(100 ms, 10 seconds)`), `maximum_attempts` (10), `interval` (5 minutes), `readiness_timeout` (10 seconds) |
| `DrainRollout` | `deadline` (2 minutes), `on_deadline` (`"disconnect_remaining"`) |

The terminal declares three contracts over one signal, the prompt probe's attempts:

```ignite,fragment
contract reachable: Availability {
  signal = promptProbe.attempts
  good = prompt_received_before_timeout
  target = 99.9%
  window = rolling(30 days)
  missing_data = "insufficient_evidence"
}

contract promptFast: ThresholdCompliance {
  signal = promptProbe.attempts
  good = prompt_received_within_threshold
  threshold = 200 ms
  target = 99%
  window = rolling(1 hour)
  missing_data = "insufficient_evidence"
}

contract customerAccess: SLA {
  signal = promptProbe.attempts
  good = prompt_received_before_timeout
  target = 99.5%
  period = calendar_month("Europe/Berlin")
  exclusions = []
  consequence = event("access_commitment_breached")
}
```

`good` names a predicate over the probe's outcome type: a name that does not exist is
[E003](../reference/diagnostics.md#e003), a predicate over another type is
[E040](../reference/diagnostics.md#e040). Texts such as `missing_data` and `on_deadline` are
closed sets. Each kind's
`validate` refuses an unknown value with [E120](../reference/diagnostics.md#e120), as it does a
target outside (0%, 100%], an unknown time zone or a non-empty `exclusions` (exclusions are not
evaluated yet, so accepting them would silently ignore them).

## The probe

`TelnetPrompt`, in `std.protocol.telnet`, produces the evidence:

```ignite,fragment
component promptProbe: TelnetPrompt {
  target = server
  interval = 10 seconds
  timeout = 2 seconds
  expected_prompt = "terminal> "
  close_after_probe = true
}
```

- Its `main` runs one attempt: resolve `target`, connect, wait for `expected_prompt`, measure
  the latency, print one `ProbeAttempt { received, latency }` line, exit 0.
- Its `machine` is a scheduler: one slot every `interval` from the target's first `Ready`, one
  probe process per slot, a deadline `timeout` after it.
- The line before the deadline is the attempt's sample; the deadline itself emits a sample
  without a line, a bad attempt. A probe that cannot run (no executable, a non-zero exit, an
  undecodable line) emits nothing and journals `missing`: the slot was expected, not executed.
- `timeout` must be under `interval` ([E123](../reference/diagnostics.md#e123)), so attempts of
  one probe never overlap; a window shorter than `interval` is
  [E124](../reference/diagnostics.md#e124).

The driver decodes each line by the signal's element type and asks every contract's `classify`
hook whether its `good` predicate holds; the two Telnet predicates live in
[`std.sli`](../reference/std/sli.md), and `meets`
compares the good attempts with the target in parts per million:

```ignite
import std.sli.{ProbeAttempt, prompt_received_before_timeout, prompt_received_within_threshold, meets}

test "outcomes and targets" {
  let slow = ProbeAttempt { received = true, latency = 350 ms }
  assert prompt_received_before_timeout(slow)
  assert !prompt_received_within_threshold(slow, 200 ms)
  assert meets(99.9%, 999, 1000)
  assert !meets(99.9%, 998, 1000)
}

test "zero traffic proves nothing" {
  assert !meets(99.9%, 0, 0)
  assert !meets(0%, 0, 0)
}
```

## Windows, missing data and standings

Each contract's `machine` is an evaluator subscribed to its signal. It re-evaluates on every
emission and at every period boundary.

| window | built by | kept as |
|---|---|---|
| `Rolling(d)` | `rolling(30 days)` | buckets of `max(interval, d / 10,000)` |
| `CalendarMonth(zone)` | `calendar_month("Europe/Berlin")` | the current month's counts, a timer at the next boundary; zones `UTC` and `Europe/Berlin` |

The population is the executed attempts in the window; missing are the expected slots that
were not executed. The status is decided in this order:

1. apply `missing_data`: `counts_as_good` or `counts_as_bad` fill the missing slots, which then
   count as executed;
2. no executed attempt, filled slots included: `InsufficientEvidence`;
3. with `insufficient_evidence`, completeness under `minimum_completeness`:
   `InsufficientEvidence`;
4. `meets(target, good, executed)`: `Compliant`, otherwise `Violated`.

The resulting `Standing` record holds `contract`, `status`, `good`, `executed`, `expected`,
`completeness`, `from`, `to`, `coverage` (the share of the window inside the evidence horizon),
`calculation = "ignite-sli/0.1"` and `version`. Slots before the target's first `Ready` are
outside the horizon, not missing.

At the end of an SLA period, a `Violated` standing fires the consequence once per contract and
period, and the journal record is its dispatch; there is no external sink yet.
`InsufficientEvidence` fires nothing and journals `consequence withheld: insufficient evidence`.

## Restart and the supervisor

`TelnetServer.machine` returns a supervisor over the server's `recovery: Restart`. Its states
are `Starting`, `Running`, `Backoff`, `Failed`, `Draining` and `Stopped`.

- The member is ready at its `listening on` line; with none within `readiness_timeout` it is
  killed and the failure counts.
- An exit that no `Kill` of the supervisor requested is a failure, exit 0 included.
- Attempt `n` counts the failures within `interval`. While `allows(restart, n)` holds the
  supervisor waits `delay(backoff, n)` and spawns again; then it gives up (`Failed`), and the
  other machines go on.

## DrainRollout and its declared disruption

`rollout <member>` on `ignite run`'s stdin replaces a server through its `DrainRollout`: build
the new generation, send SIGTERM to the old process, wait for its `released` line, spawn the
new one, and let the old one finish its sessions until `deadline`, after which `on_deadline`
disconnects the rest. A failed build aborts before the old process is touched.

The listener is closed before it is rebound, so the rollout is not zero-downtime, and the
model says so instead of hiding it:

```text
$ ignite explain examples/terminal/terminal.design.ignite Terminal.server.rollout
Terminal.server.rollout: machine Roll of Terminal.server
  id = "Terminal.server.rollout"
  member = "Terminal.server"
  policy = DrainRollout { deadline: 2 minutes, on_deadline: "disconnect_remaining" }
  state = Idle
  ...
disruption: connections refused between listener close and rebind
```

The journal records the measured disruption, from the release to the new process's `Ready`.

## The machines are pure

Supervisor, scheduler, evaluator and rollout are the four variants of `std.control.Machine`.
Each is a reducer `step(machine, event) -> Step` in Ignite, with no IO: adapters turn its
effects into processes, timers and probe attempts and feed the results back as events. That is
why a journal is enough to replay every decision; see
[The compile-time evaluator](evaluator.md) and the
[`std.control` reference](../reference/std/control.md).

Next: [One substrate, two targets](substrate.md).
