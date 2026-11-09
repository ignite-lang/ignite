# The run-time model

A compiled program serves. Something else has to start it, restart it when it fails, probe
it and judge the evidence against its contracts. In Ignite that is the run-time driver of
`ignite run` and `ignite host`. This page explains how the driver decides, what it records,
and why a run can be replayed from its record alone. The commands are in
[Plan, explain, run and replay](../guide/operate.md) and [Local apply](../guide/apply.md).

## The decision loop

The design describes a runtime that is a deterministic decision engine around an effect
boundary:

1. An ordered stream of events enters a reducer.
2. The reducer returns its next state and the effects it wants carried out.
3. A journal records each decision and each intent to act before anything happens.
4. Adapters carry out the effects and turn what they observe back into events.

What is built follows that shape. The reducers are pure Ignite functions in
[`std.control`](../reference/std/control.md), `step(machine, event) -> Step`, and the
driver executes them in the [compile-time evaluator](evaluator.md) rather than compiling
them. The driver keeps one queue of events: a process started, printed its ready line or
exited, a probe attempt finished, a timer fired, an operator typed `rollout`. Each event is
journaled, then stepped through the machines it is routed to. Each effect a step returns,
such as spawn, kill, set a timer or emit a signal, is journaled, then dispatched.

The adapters run on their own threads. They start processes, run probe attempts, fire
timers and read the operator's commands, and feed what they observe back into the queue.
Reducers never do IO, and adapters never decide. Every decision is therefore a pure
function of the journaled events.

## Four machines

A member's kind chooses its machine through its `machine` hook
([Components](components.md#machine-gives-the-member-its-control)). There are four, and a
program cannot add a fifth:

- The **supervisor** starts a native process and waits for its `listening on` line. It
  counts an exit that it did not request as a failure, exit 0 included. It restarts the
  member under its `Restart` policy, and gives up when the policy does. The other machines
  go on.
- The **probe scheduler** opens a slot every `interval` from the target's first readiness.
  It starts one short-lived probe process per slot, with a deadline of `timeout`.
- The **evaluator** of a contract counts the verdicts of its signal in its window. It
  publishes a standing at every emission and at every period boundary.
- The **rollout** replaces a member's process under its `DrainRollout` policy, described
  below.

The [`std.control` reference](../reference/std/control.md) has each machine's events,
effects and states.

## From an attempt to a standing

The evidence travels through the machines in small, typed steps. The probe process connects
to its target and prints one line, its outcome in the model encoding. The scheduler emits
that line on the probe's signal as a sample of its slot. A deadline without a line emits a
sample without one, a bad attempt. A probe that could not run at all emits nothing, and the
slot is journaled as missing.

The driver decodes each sample by the signal's element type. It asks each subscribed
contract's `classify` hook whether the contract's `good` predicate holds, and routes the
verdict to that contract's evaluator. The evaluator turns verdicts and missing slots into a
standing, by the rules of [Contracts and policies](contracts.md#zero-traffic-proves-nothing).
At the end of an SLA period, a violated standing fires the consequence once.

## Time

Reducers read no clock. Every event carries `at`, the time since the journal's epoch, and
the driver owns the only clock. With `--clock real`, `at` comes from the monotonic clock.
With `--clock manual`, time moves only when the operator types `advance`, and every timer
that falls due fires in order. The reducers cannot tell the two clocks apart, so a test can
drive the same decisions without waiting for them.

## The journal

The journal is the run's record, one JSON line per record in the state directory. Its
header holds the full system model. Every other record is a run segment, an event, an
effect, a signal, a standing, a fired consequence or a reducer error, numbered without gaps.
Each effect is synced to disk before it is dispatched. After a crash, the journal therefore
says what the driver intended, even if the effect never happened. A second run on the same
state with the same model appends a new segment. The record shapes are in
[System declarations](../reference/systems.md#the-journal).

## Replay

Because every decision is a pure step over journaled events, the journal is enough to
reproduce the run. `ignite replay` checks the header against the current plan, re-steps the
journaled events, and compares every effect, signal, standing and consequence it gets with
the journal, byte for byte. The first difference is
[E127](../reference/diagnostics.md#e127), with its sequence number. What a real process
contributes, such as a pid, a latency or an exit code, enters only through events, so replay
is exact.

The same reducers also compile. A test in the repository steps them in the evaluator, on
the Rust target and on the LLVM target, and the three must agree.

## Failure stays local

A reducer that fails, on an assertion, an overflow or running out of fuel, is
[E128](../reference/diagnostics.md#e128). That machine stops, the others go on, and the run
exits 1. A crash of the driver itself leaves no orphan: every child is started in the run's
process group and dies with the driver. `stop`, SIGINT or SIGTERM drain every process with
SIGTERM, wait up to the member's drain deadline, kill what is left and exit.

## Rollout and its declared disruption

`rollout <member>` on the run's stdin starts the member's rollout machine. It builds the
next generation first, so a failed build aborts before the old process is touched. It then
sends SIGTERM to the old process, waits until it has released its listener, and spawns the
new one. The old process finishes its sessions until the policy's `deadline`, and
`on_deadline` disconnects the rest.

The listener is closed before it is rebound, so the rollout is not zero-downtime. The model
says so instead of hiding it:

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

The journal records the measured disruption, from the release to the new process's
readiness. [Replace a server with a rollout](../howto/rollout.md) shows the steps.

## The host

`ignite run` runs an assembly in the foreground and stops with it. `ignite host` keeps the
same machines in a long-running process that owns one state directory. `ignite apply`
submits a saved plan to it. The client may exit or retry, and the host keeps the work and
the record. The host follows the same rule as the run, with its own journal:

- Acceptance is synced to disk before `apply` gets an answer, and every effect intent before
  the host carries it out.
- A plan is bound to the digests of its sources, its compiler and its artifacts. An input
  that changed between plan and apply rejects the plan. Apply never rebuilds behind your back.
- The revision is committed only when every action converged. A failed operation keeps the
  previous committed revision and reports the partial state. It does not claim a rollback.
- Retrying an accepted plan returns the same operation, never a second dispatch.
- After a crash, the restarted host reconciles every intent without a recorded completion.
  A process it cannot prove it owns is neither signalled nor adopted. The operation becomes
  `recovery_required`, and `ignite inspect` shows the evidence and the next action.
- At start, the host checks its journal: a torn last record is repaired and journaled, and
  an altered record or a cut journal stops the host before it writes anything.

When no corrective plan resolves a recovery, `ignite recover` abandons the operation. It
stops every instance the state owns and restarts the committed revision. It refuses when a
process it cannot account for runs from the state.
[Recover a host](../howto/recover.md) shows the steps.

Next: [The compile-time evaluator](evaluator.md).
