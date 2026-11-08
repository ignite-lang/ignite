# std.control

The pure reducers that `ignite run` steps. A machine is a value, an event is a value, and
`step` is a pure function from both to the next machine and the effects to dispatch.
Reducers read no clock: every event carries `at`, a `Duration` since the start of the run's
journal.

The driver of `ignite run` knows these types, the hook names `machine`, `validate` and
`classify`, and the functions `step` and `companions`. The module also holds the `Transition`
reducer that orders the changes of one `ignite apply`. `ignite apply` calls the constructors
`member_action` and `transition` and steps `Transition` with `transit`, see
[Transition](#transition-apply).

- Layer: general. It imports only the leaf modules [std.sli](sli.md),
  [std.policy](policy.md) and [std.time](time.md), so that every kind whose `machine` hook
  returns a `Machine` can import it without an import cycle.
- Source: [`std/control.ignite`](https://github.com/ignite-lang/ignite/blob/main/std/control.ignite).
- Import: `import std.control.{Machine, Supervisor, Event, Effect, step}`.

You rarely call this module yourself. A component kind returns one of its machines from its
`machine` hook (`TelnetServer.machine` returns a supervisor), and `ignite run` does the
stepping. See [Plan, explain, run and replay](../../guide/operate.md) and
[Local apply](../../guide/apply.md).

## The reducer interface

```ignite,fragment
enum Machine {
  Supervise(Supervisor),
  Schedule(ProbeScheduler),
  Evaluate(Evaluator),
  Roll(Rollout)
}

type Step {
  machine: Machine
  effects: List<Effect>
}

type Event {
  at: Duration
  kind: EventKind
}

fn step(machine: Machine, event: Event) -> Step
fn companions(machine: Machine) -> List<Machine>
```

- `Machine` is a closed set: a program cannot define a new kind of machine. Machine ids are
  `<member path>.<role>`, such as `Terminal.server.supervisor`.
- `step` steps `machine` over `event` and returns the machine after it and the effects to
  dispatch, in order. The driver journals the event before the step and every effect after.
- `companions` gives the machines a hook's machine brings with it: a supervisor whose member
  has a `DrainRollout` policy brings the member's rollout machine. The driver calls it for
  every machine a `machine` hook returned.

A hook has the shape `fn K.machine(self: K, id: Text) -> Machine`, where `id` is the member
path. Any other shape is E109. A reducer that fails on an assertion, an overflow or running
out of fuel stops that machine only. The run continues and exits 1 (E128).

### Events

```ignite,fragment
enum EventKind {
  Start(Duration),
  Started(Int, Int),
  Ready(Int),
  Exited(Int, Int),
  Signalled(Int, Int),
  SpawnFailed(Text),
  AttemptDone(Int, Text),
  AttemptFailed(Int, Text),
  Timer(Int),
  Emitted(Text, Verdict),
  Built(Int),
  BuildFailed(Text),
  Released,
  Control(Text)
}
```

| event | meaning |
|---|---|
| `Start(epoch)` | a run starts or resumes; carries the journal's epoch (a `Duration` since 1970-01-01T00:00:00Z) and reaches every machine |
| `Started(pid, generation)` | a process of the member started |
| `Ready(pid)` | the process printed its `listening on` line |
| `Exited(pid, code)`, `Signalled(pid, signal)` | the process ended |
| `SpawnFailed(reason)` | the process could not be started |
| `AttemptDone(id, line)`, `AttemptFailed(id, reason)` | a probe attempt finished with its line (decoded by the signal's element type before it is fed), or failed without a result |
| `Timer(id)` | a timer the machine set fired |
| `Emitted(signal, verdict)` | an emission on a signal the machine subscribes to, with the [`Verdict`](sli.md#samples-and-verdicts) of the machine's contract |
| `Built(generation)`, `BuildFailed(reason)` | a rollout's build finished |
| `Released` | the old process released its listener during a rollout |
| `Control(line)` | a control line from `ignite run`'s standard input, such as `stop` or `rollout` |

Lifecycle events of a member go to its supervisor, its rollout machine and every scheduler
probing that member.

### Effects

```ignite,fragment
enum Effect {
  Spawn(Text, Int),
  Kill(Text, Int, Int),
  Attempt(Text, Int),
  Abort(Text, Int),
  SetTimer(Int, Duration),
  CancelTimer(Int),
  Emit(Text, Sample),
  Publish(Standing),
  Fire(Fired),
  Build(Text),
  AwaitRelease(Text),
  Note(Text)
}
```

| effect | what the driver does |
|---|---|
| `Spawn(member, generation)` | start the member's executable of that generation |
| `Kill(member, pid, signal)` | send a POSIX signal to one process of the member |
| `Attempt(member, id)`, `Abort(member, id)` | run one probe attempt, or end its process |
| `SetTimer(id, instant)`, `CancelTimer(id)` | arm a timer for an absolute instant, or disarm it |
| `Emit(signal, sample)` | emit a [`Sample`](sli.md#samples-and-verdicts) on a signal |
| `Publish(standing)` | record a contract's standing |
| `Fire(fired)` | record a consequence that fired |
| `Build(member)` | build the member's next generation |
| `AwaitRelease(member)` | wait until the old process released its listener |
| `Note(text)` | a journal line, nothing else |

Members and signals are named by their path text (`Terminal.server`,
`Terminal.promptProbe.attempts`).

## Supervisor

```ignite,fragment
type Supervisor {
  id: Text
  member: Text
  restart: Restart
  rollout: Option<DrainRollout> = None
  has_listener: Bool = true
  state: SupervisorState = Stopped
  generation: Int = 1
  pid: Option<Int> = None
  failures: List<Duration> = []
  attempt: Int = 0
  timer: Option<Int> = None
  kill_requested: Bool = false
}

enum SupervisorState { Starting, Running, Backoff, Failed, Draining, Stopped }
```

The supervisor of one native-process member. It starts the member, waits for its
`listening on` line, restarts it under its [`Restart`](policy.md) policy and gives up when the
policy does.

- `Start` spawns the member afresh. `Started` arms the readiness timer. `Ready` disarms it. A
  member without a listener is ready at `Started`.
- No `Ready` within `readiness_timeout` kills the process and counts a failure.
- An exit or signal that no `Kill` of this machine requested is a failure, exit 0 included,
  and so is a failed spawn.
- After a failure the attempt number is the number of failures within the policy's
  trailing `interval`, this one included. If the policy `allows` it, the supervisor waits the
  backoff's `delay` and spawns again. Otherwise it is `Failed` and notes `gave_up`. Other
  machines go on.
- `Control("stop")` drains the process with SIGTERM. Its exit is then no failure.

## Probe scheduler

```ignite,fragment
type ProbeScheduler {
  id: Text
  member: Text
  target: Text
  signal: Text
  interval: Duration
  timeout: Duration
  target_ready: Bool = false
  next_id: Int = 1
  slot: Option<Duration> = None
  in_flight: Option<Int> = None
  in_flight_slot: Option<Duration> = None
  deadline_timer: Option<Int> = None
  slot_timer: Option<Int> = None
}
```

The scheduler of one probe member. Slots open every `interval` from the target's first
`Ready` on. Each slot dispatches one attempt with a deadline of `timeout`.

- A result before the deadline is emitted on `signal` as `Sample { slot, line = Some(line) }`,
  `slot` being the slot the attempt was dispatched for.
- The deadline aborts the attempt and emits `Sample { slot, line = None }`, a bad attempt.
- A failed attempt is missing evidence: no executable, an exit status other than 0, an
  undecodable line. It emits nothing and notes `missing`.
- `Start` forgets the schedule. The target's next `Ready` reopens it.

## Evaluator

```ignite,fragment
type Evaluator {
  id: Text
  contract: Text
  signal: Text
  interval: Duration = 0 ms
  target: Ratio
  window: Window
  missing_data: MissingData
  minimum_completeness: Ratio
  consequence: Option<Text> = None
  epoch: Duration = 0 ms
  horizon: Option<Duration> = None
  buckets: List<Bucket> = []
  current: Option<Bucket> = None
  sum_good: Int = 0
  sum_executed: Int = 0
  period_timer: Option<Int> = None
  fired: List<Duration> = []
}

type Bucket {
  from: Duration
  good: Int = 0
  executed: Int = 0
}
```

The evaluator of one [contract](contract.md). It counts each `Verdict` of its signal in the
bucket of its slot, keeps the window's counts, and publishes a `Standing` at every emission
and every period boundary. The driver has applied the contract's `classify` to the decoded
line before the `Verdict` arrives.

- A `Rolling(span)` window keeps its counts in buckets of `max(interval, span / 10,000)`.
- A `CalendarMonth(zone)` window keeps the current period's counts and sets a timer at the
  next month boundary in `zone`. At the boundary it publishes the closed period. An `SLA`
  fires its consequence once for a closed period that is `Violated`, and withholds it with a
  note on insufficient evidence.

```ignite,fragment
type Standing {
  contract: Text
  status: Status
  good: Int
  executed: Int
  expected: Int
  completeness: Ratio
  from: Duration
  to: Duration
  coverage: Ratio
  calculation: Text = "ignite-sli/0.1"
  version: Int = 0
}

enum Status { Compliant, Violated, InsufficientEvidence }

type Fired {
  contract: Text
  consequence: Text
  period_from: Duration
  period_to: Duration
  standing_seq: Int
}

fn standing(path: Text, target: Ratio, missing_data: MissingData, minimum_completeness: Ratio, good: Int, executed: Int, expected: Int, from: Duration, to: Duration, coverage: Ratio) -> Standing
fn expected_slots(horizon: Duration, interval: Duration, from: Duration, to: Duration) -> Int
```

`standing` decides the status in this order:

1. The missing slots are filled by the `missing_data` rule (`counts_as_good`,
   `counts_as_bad`).
2. No executed attempt is `InsufficientEvidence`.
3. Completeness under `minimum_completeness`, when missing data is insufficient evidence, is
   `InsufficientEvidence`.
4. Otherwise [`meets`](sli.md) decides: `Compliant` or `Violated`.

| field | value |
|---|---|
| `completeness` | the executed share of the expected slots before filling |
| `coverage` | the share of the window inside the evidence horizon |
| `version` | 0 from the reducer; the driver fills it with the journal sequence number of the standing record |
| `standing_seq` of a `Fired` record | filled by the driver with the sequence number of the standing it rests on |
| `expected_slots` | the slots of `interval` from the horizon within `from` to `to` |

## Rollout

```ignite,fragment
type Rollout {
  id: Text
  member: Text
  policy: DrainRollout
  state: RolloutState = Idle
  generation: Int = 1
  old_pid: Option<Int> = None
  new_pid: Option<Int> = None
  released_at: Option<Duration> = None
  deadline_timer: Option<Int> = None
  start_first: Bool = false
}

enum RolloutState { Idle, Building, Draining, Starting, Retiring, Done, Aborted }
```

The rollout of one member under its [`DrainRollout`](policy.md) policy, started by
`Control("rollout")`:

1. `Build` the next generation. `BuildFailed` aborts here, and the old process keeps serving.
2. `Kill` the old process with SIGTERM: it drains, and `AwaitRelease` waits until it has
   released its listener.
3. `Spawn` the new generation and wait for its `Ready`. The interval from `Released` to
   `Ready` is noted as the disruption.
4. The old process retires when it exits. If it still runs at the policy's `deadline`,
   `on_deadline` applies: `disconnect_remaining` kills it.

`Control("rollout start_first")` starts the new process while the old one keeps serving, and
drains the old one only after the new one is ready. A new process that never becomes ready
leaves the old one serving.

## Journal segments and constants

```ignite,fragment
type Segment {
  start: Duration
  stop: Option<Duration> = None
  clock: Text
  generation: Int
  declared: List<Text> = []
}
```

`Segment` is one record of the journal's `segment` kind: a run's start, resume or stop, the
clock mode, the generation, and the declared members the run cannot build.

| constant | value | kind |
|---|---|---|
| `SIGKILL` | 9 | signal |
| `SIGTERM` | 15 | signal |
| `BACKOFF_TIMER` | 1 | supervisor timer |
| `READINESS_TIMER` | 2 | supervisor timer |
| `SLOT_TIMER` | 0 | scheduler timer; deadline timers carry the attempt id |
| `PERIOD_TIMER` | 1 | evaluator timer |
| `DEADLINE_TIMER` | 1 | rollout timer |
| `BUCKETS_PER_WINDOW` | 10000 | |

## Transition (apply)

`Transition` orders the changes of one `ignite apply`. The driver computes the plan's member
actions and steps this reducer. The reducer decides what happens next. The driver only
executes it and reports back. It is pure and reads no clock.

```ignite,fragment
enum ActionKind { Add, Replace, Remove, Detach, Keep }

type MemberAction {
  member: Text
  kind: ActionKind
  owned: Bool
  cutover: Bool = false
  depends_on: List<Text> = []
}

enum TransitionPhase { Planned, Stopping, Switching, Launching, Converging, Committed, Abandoned }

type Transition {
  id: Text
  actions: List<MemberAction>
  start_order: List<Text>
  stop_order: List<Text>
  phase: TransitionPhase = Planned
  current: Option<Text> = None
  stopped: List<Text> = []
  ready: List<Text> = []
  failure: Option<Text> = None
}

enum TransitionEvent {
  Begin,
  MemberStopped(Text),
  Switched,
  MemberReady(Text),
  MemberGaveUp(Text),
  Converged,
  EffectFailed(Text, Text)
}

enum TransitionEffect {
  StopMember(Text),
  SwitchRevision,
  StartMember(Text),
  CutOver(Text),
  Commit,
  Abandon(Text, Text)
}

type TransitionStep {
  transition: Transition
  effects: List<TransitionEffect>
}

fn transit(t: Transition, event: TransitionEvent) -> TransitionStep
fn transition(id: Text, actions: List<MemberAction>, start_order: List<Text>, stop_order: List<Text>) -> Transition
fn member_action(member: Text, kind: Text, ownership: Text, cutover: Bool, depends_on: List<Text>) -> MemberAction
fn action_kind(text: Text) -> ActionKind
```

The sequence:

1. Every owned member to remove, and every owned member to replace without a draining
   cutover, is stopped in `stop_order`, dependents before their providers, one at a time,
   each awaiting its `MemberStopped`. A provider whose dependent cuts over is not stopped
   first: it still serves that dependent.
2. `SwitchRevision` hands the running instances that stay to the desired revision.
3. Every owned member to add or replace starts in `start_order`, one at a time, each only
   once every provider it depends on that this transition starts is ready: `StartMember` for
   an addition, `CutOver` for a replacement through its rollout.
4. When the last one is ready the transition converges, and the driver's `Converged` gives
   `Commit`.

| cause | the transition is abandoned with |
|---|---|
| a desired member gives up | `Abandon("gave_up", member)` |
| the driver could not carry out an effect (`EffectFailed`) | its reason |
| members whose providers can never be ready | `ordering_cycle` |

A member that is not owned, and a member to detach or keep, is never stopped or started.
`member_action` and `transition` are the constructors the driver calls with the plan's texts.
`action_kind` maps `add`, `replace`, `remove` and `detach`, and any other text to `Keep`.

## Examples

The reducers are tested with synthetic event lists and chosen `at` values, no clock. These
assertions are taken from the module's own tests:

```ignite
import std.control.{Machine, Supervisor, SupervisorState, Event, EventKind, Effect, Rollout, step, companions, READINESS_TIMER}
import std.policy.{Restart, DrainRollout}

test fn at(ms: Int, kind: EventKind) -> Event = Event { at = ms * 1 ms, kind }

test fn server() -> Supervisor = Supervisor {
  id = "T.server.supervisor"
  member = "T.server"
  restart = Restart { maximum_attempts = 10, interval = 5 minutes }
}

// Step `machine` through `events`, collecting every effect.
test fn run(machine: Machine, events: List<Event>) -> (Machine, List<Effect>) {
  var current = machine
  var effects = []
  for event in events {
    let result = step(current, event)
    current = result.machine
    effects.append(result.effects)
  }
  (current, effects)
}

test fn state_of(machine: Machine) -> SupervisorState = match machine {
  Supervise(s) => s.state
  _ => Stopped
}

test "a supervisor spawns at start, arms readiness at started and runs at ready" {
  let (m1, e1) = run(Supervise(server()), [at(0, Start(1767225600 seconds))])
  assert e1 == [Spawn("T.server", 1)]
  assert state_of(m1) == Starting
  let (m2, e2) = run(m1, [at(10, Started(42, 1))])
  assert e2 == [SetTimer(READINESS_TIMER, 10010 ms)]
  let (m3, e3) = run(m2, [at(20, Ready(42))])
  assert e3 == [CancelTimer(READINESS_TIMER)]
  assert state_of(m3) == Running
}

test "a supervisor with a rollout policy brings the member's rollout machine" {
  var s = server()
  assert companions(Supervise(s)) == []
  s.rollout = Some(DrainRollout { deadline = 30 seconds })
  assert companions(Supervise(s)) == [Roll(Rollout { id = "T.server.rollout", member = "T.server", policy = DrainRollout { deadline = 30 seconds } })]
}
```

A transition replacing a server through its rollout, with a probe that depends on it:

```ignite
import std.control.{Transition, TransitionEvent, TransitionEffect, TransitionPhase, transit, transition, member_action}

test fn transit_all(t: Transition, events: List<TransitionEvent>) -> (Transition, List<TransitionEffect>) {
  var current = t
  var effects = []
  for event in events {
    let result = transit(current, event)
    current = result.transition
    effects.append(result.effects)
  }
  (current, effects)
}

test "a replacement with a rollout cuts over, one without stops first and starts after the switch" {
  let t = transition("c", [
    member_action("T.probe", "replace", "owned", false, ["T.server"]),
    member_action("T.server", "replace", "owned", true, [])
  ], ["T.server", "T.probe"], ["T.probe", "T.server"])
  let (t1, e1) = transit_all(t, [Begin, MemberStopped("T.probe"), Switched, MemberReady("T.server"), MemberReady("T.probe"), Converged])
  assert e1 == [StopMember("T.probe"), SwitchRevision, CutOver("T.server"), StartMember("T.probe"), Commit]
  assert t1.phase == Committed
}
```
