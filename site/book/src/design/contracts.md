# Contracts and policies

A contract says what the system promises. A policy says what may be done to keep the
promise. Both are declarations, checked at compile time and evaluated at run time by pure
control machines that `ignite run` steps. This page explains the ideas behind contracts and
policies, and why missing evidence is never counted as success. It uses
[`examples/terminal/terminal.design.ignite`](https://github.com/ignite-lang/ignite/blob/main/examples/terminal/terminal.design.ignite).
The commands are in [Plan, explain, run and replay](../guide/operate.md).

## Indicators, objectives and agreements

Design section 4.1 separates three layers:

| layer | adds |
|---|---|
| indicator (SLI) | an eligible population, the good outcomes, the measurement source, the accounting rules |
| objective (SLO) | a target and a window |
| agreement (SLA) | contractual scope, period, exclusions, reporting and consequences |

The standard library ships one kind per layer above the indicator:
[`Availability` and `ThresholdCompliance`](../reference/std/contract.md#contract-kinds) are
objectives over a rolling or calendar window, and `SLA` is an agreement over a calendar
period with a consequence. The indicator is the signal of a probe and a predicate over its
outcome.

## A contract names a predicate over evidence

The terminal promises that its prompt is reachable:

```ignite,fragment
contract reachable: Availability {
  signal = promptProbe.attempts
  good = prompt_received_before_timeout
  target = 99.9%
  window = rolling(30 days)
  missing_data = "insufficient_evidence"
}
```

`signal` is the output of the probe `promptProbe`, one value per attempt. `good` names a
function from that value to `Bool`, so what counts as a good outcome is code with tests,
not a string. A probe kind chooses its own outcome type. The Telnet probe reports a
`ProbeAttempt`, and the todo example's HTTP probe reports whether it was answered, the status and the latency
([Write a probe and a contract](../howto/probe-contract.md)). A predicate over another type
is [E040](../reference/diagnostics.md#e040).

Each kind's `validate` refuses what its type cannot: a target outside (0%, 100%], an unknown
time zone, an unknown `missing_data` text. It also refuses a non-empty `exclusions`, because
exclusions are not evaluated yet, and accepting them would silently ignore them. The terminal
declares two more contracts over the same signal; all three are in
[`std.contract`](../reference/std/contract.md#in-a-declaration).

## Zero traffic proves nothing

A standing is `compliant`, `violated` or `insufficient_evidence`. It carries its interval,
its completeness and its calculation version. Two rules from the design shape the
evaluation: "Zero traffic does not automatically prove compliance. Missing telemetry does
not become a successful sample."

So an attempt that did not run is not a bad attempt and not a good one. It is missing, and
the contract's `missing_data` decides what missing means. With `insufficient_evidence`, a
window without enough executed attempts has no verdict at all. The target check holds the
same line:

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

The order in which completeness, missing data and the target decide a status is in
[`std.control`](../reference/std/control.md#evaluator).

## Policy authority

Section 4.2: "Objectives report whether intent is met. Policies decide what actions are
permitted in response." A contract never restarts anything. A policy bounds what a machine
may do, and the machine acts only inside those bounds. The standard library ships two, in
[`std.policy`](../reference/std/policy.md):

- `Restart` bounds the supervisor: the backoff between attempts, how many failures within an
  interval it tolerates, and how long a new process may take to become ready;
- `DrainRollout` bounds a replacement: how long the old process may finish its sessions, and
  what happens to the sessions left at the deadline.

A policy also states what it costs. A drain rollout closes the listener before the new
process binds it, and the model says so instead of claiming zero downtime
([The run-time model](runtime.md#rollout-and-its-declared-disruption)).

An SLA consequence emits a typed breach event. The design states that Ignite does not infer
payment obligations or execute settlement. Today the journal record is the consequence's
only dispatch.

Next: [The run-time model](runtime.md).
