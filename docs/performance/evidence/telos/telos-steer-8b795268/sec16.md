## 16 Slice 12: run and control (F3)

Contract reviewed and accepted by the steering session on 2026-11-02, with fifteen
amendments (review of 04:10, A1 to A15) applied. Sources: vision 4.1 row 12, invariants 5, 6
and 9, T1, T4, D1, D5, D7; actors.md R8; design 2, 3.4, 4.1 to 4.3, 6.2, 7, 7.1, 7.4, 10.1,
11.7, 11.8, 11.10 (Rollout, Recovery, Contracts); `docs/slice.md` 6.1, 14.5, 14.8, 15.4 to
15.8 and 15.12 (O18, O19, O25); `std/contract.ignite`, `std/policy.ignite`,
`std/probe.ignite`, `std/deploy.ignite`, `std/protocol/telnet.ignite`;
`examples/terminal/terminal.design.ignite`. Decisions O27 to O44 are recorded in 16.19.

The slice adds two commands:

- `ignite run` starts the processes an assembly hosts, supervises them, runs the probes,
  evaluates the contracts and writes every decision to a journal.
- `ignite replay` re-derives every decision from that journal.

Vision 4.1 (amended by steering, A1): the reducers are stepped by the evaluator inside
`ignite run`, and are compiled on both targets in the trace fixture of item 10. The
controller is written as reducers in the shape of actors.md R8, so that wave C replaces the
runtime under them, not the reducers.

### 16.0 Acceptance

Baseline first (16.1). Then the row-12 items.

1. The terminal system runs under `ignite run`.
   - The command: `ignite run examples/terminal/terminal.design.ignite --assembly local
     --port Terminal.ingress=0 --clock manual`.
   - It builds `terminald` and `promptProbe`, starts `terminald`, and journals `started`,
     then `ready` (16.7).
   - A TCP client on the reported port reads `terminal> `, and `echo hello` answers `hello`.
   - The `client` member is listed as declared, not buildable (16.3, A10).
   - The control line `stop` ends the run with exit 0. No process the run started is left,
     checked through `/proc`.
   - One test runs the same with `--target llvm`.
2. Killing `terminald` restarts it within the policy's backoff.
   - Manual clock: after `ready`, the test sends SIGKILL to the journaled pid.
   - The journal shows `exited`, then a `restart` effect with `attempt 1` and `delay 100 ms`.
   - `advance 99 ms` starts nothing. `advance 1 ms` journals `started` at exactly
     `exited.at + 100 ms`, the instant of the `Spawn` dispatch.
   - A second kill gives `delay 200 ms`.
   - Real clock (one test): `started.at - exited.at` lies in `[100 ms, 500 ms]`.
   - A fixture with `maximum_attempts = 2` journals `gave_up` on the third failure inside
     `interval`. The run goes on, and `stop` then exits 1.
3. A probe timeout is visible in the journal and in the contract's standing.
   - Manual clock: one good attempt is journaled.
   - The test sends SIGSTOP to `terminald`, so connects queue in the backlog and no prompt
     arrives.
   - The next scheduled attempt is dispatched. `advance 2 seconds` journals `probe timeout`,
     a bad attempt.
   - The next `standing` record of `Terminal.reachable` reads `violated` with `good 1` and
     `executed 2`.
   - The control line `status` prints the same standing.
   - After SIGCONT, attempts are good again. The 30-day window keeps `violated`.
   - This is the only test that pays a settle wait (16.4, A2).

Contract semantics (design 4.1, 11.7):

4. Zero executed attempts give `insufficient_evidence`, never `compliant`.
   - `std.contract.meets(_, 0, 0)` is `false` (16.10, O32).
   - A probe that cannot run (its executable is missing, or its output does not parse) is
     missing evidence, not a bad attempt.
   - Completeness below `minimum_completeness` gives `insufficient_evidence`.
   - Each case has an inline test over synthetic events. The zero-attempt case also has an
     end-to-end test: a run stopped before the first slot.
5. `ThresholdCompliance` and `SLA` evaluate from the same signal.
   - The test of item 3 starts with `--epoch 2026-10-31T22:50:00Z`. After the timeout it
     advances across the Europe/Berlin month boundary (2026-10-31T23:00Z).
   - It journals the `standing` of the closed period and one `consequence`,
     `access_commitment_breached`.
   - A second run with only good attempts crosses the same boundary and journals no
     consequence.
   - Inline tests over synthetic events cover the rest:
     - no consequence for `compliant` or `insufficient_evidence`;
     - none twice for one period;
     - period boundaries in months with a DST change (16.11).
6. `DrainRollout` works (A12: the last unit of P2 and P4).
   - The control line `rollout Terminal.server` rebuilds `terminald` and drains the old
     process: its listener closes and prints `released`, and its sessions keep working.
   - The run then starts the new process.
   - At `deadline` (manual clock), the old process gets SIGKILL if sessions remain
     (`disconnect_remaining`).
   - The journal records the declared disruption with its measured interval (16.12).
   - An open session on the old process answers `echo` until the deadline.

Determinism and time:

7. Every journal written by the end-to-end tests replays.
   - `ignite replay` re-derives every effect and standing record byte for byte and exits 0
     (16.15).
   - A journal with one effect record altered exits with E127, naming its `seq`.
8. Two manual-clock runs of one scenario give equal journals after masking the `pid` and
   probe `latency` values, the only values a real process contributes.
9. No test sleeps for a policy or probe duration.
   - Probe timeouts, backoff, windows and calendar periods are driven by the manual clock
     (16.4).
   - Tests assert the `at` of `Ready`, `AttemptDone` and `Started` only as "the instant of
     the dispatch" (A2).
   - All `ignite run` end-to-end tests together add at most 6 s of wall time to
     `crates/ignite/tests` (O41).
10. The reducers agree across the evaluator and both targets.
    - A trace fixture steps each reducer of 16.6 over one event list in the evaluator, in a
      Rust-target build and in an LLVM-target build.
    - The debug texts are equal (this extends 15.0 item 8).

Records:

11. `ignite plan` of the design file adds `machines`, `observations` and the edge classes of
    16.14.
    - The version stays `ignite-model/0.1` (O13, additive).
    - The golden is re-blessed, with the diff explained.
    - The terminal and todo model goldens change only by empty `machines` and
      `observations` keys.
    - Every Rust and LLVM build golden is byte-identical, except programs that use the new
      builtins (16.13).
12. Invariant 9 (16.1).
    - Measured with the interleaved pairs of 15.1 against the slice-12 start tree:
      - gate cold and warm;
      - the warm `cargo test --workspace` step;
      - `todod` on both targets, dev and release;
      - `ignite check` and `ignite plan` of the design file.
    - A rise over 10 % is a deviation to explain; O26 applies.
    - The warm test step stays under slice 11b's recorded number: 10.8 s median at load 6
      to 9 (`AGENTS.md`).
    - New absolute numbers:
      - `ignite run` from start to the first `ready` (warm build);
      - `ignite replay` of a 10,000-record journal;
      - the evaluator test of A7 (16.10): 10,000 emissions into a 30-day window.
13. Every diagnostic E120 to E129 has a fixture (16.17).

### 16.1 Baseline and measurement

- The baseline tree is `/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/s12-before`.
  - It is an rsync copy of `git ls-files -co --exclude-standard` of the working tree at
    slice-12 start (741 files), made by the coordinator.
  - It is not a `git archive`, because the slice-11 and 11b state is uncommitted. The copy
    replaces the slice-11 close commit.
  - It has its own target dir.
- No P0: the slice-11 and 11b rows in `AGENTS.md` are the reference.
- P4 re-measures both trees in pairs, as 15.1 prescribes:
  - one run per tree, alternating, at least 3 pairs;
  - the 1-minute load recorded for every run;
  - a pair dropped when its two loads differ by more than 5;
  - the median of the pair ratios reported next to the absolute times.
- New end-to-end test files of this slice are timed per file in the same pairs. The before
  tree has none, so their absolute time is reported.

### 16.2 Run-time architecture

| part | where | language | role |
|---|---|---|---|
| reducers | `std/control/*.ignite` | Ignite | supervisor, probe scheduler, contract evaluator, rollout: pure fns `step(machine, event) -> Step` (16.6) |
| domain kinds and hooks | `std/contract.ignite`, `std/policy.ignite`, `std/probe.ignite`, `std/protocol/telnet.ignite`, `std/time.ignite` | Ignite | `K.machine`, `K.validate`, the probe's `K.main`, calendar arithmetic |
| driver (the supervisor process) | `crates/ignite/src/run/` | Rust | event loop, adapters (process, probe, timer, build, journal, clock, control input), routing; it knows only the `std.control` types |
| evaluator at run time | `crates/ignite-eval` | Rust | steps the reducers (O27); decodes adapter results into values |
| substrate | `crates/ignite-io` (and the `ignite-rt` adapter) | Rust | monotonic clock, reference endpoints, drain on SIGTERM (16.13); nothing about supervision |

The supervisor process:

- The supervisor process is the `ignite run` process itself, in the foreground (design
  10.1). It is not a compiled Ignite program.
- Its children are the built executables: one per `NativeProcess` member, and one
  short-lived probe process per probe attempt.
- Each child is started in the run's process group with `PR_SET_PDEATHSIG(SIGKILL)`, so that
  a crash of `ignite run` leaves no orphan.
- prctl(2) ties that signal to the thread that forked, not to the process (A3). So the
  process adapter spawns every child, probes included, from one spawner thread that lives
  for the whole run. Other threads send it spawn requests.
- The run stops on SIGINT, on SIGTERM and on the control line `stop`:
  1. It journals `stopping`.
  2. It drains every process with SIGTERM.
  3. It waits up to each member's `DrainRollout.deadline` (10 s when the member has none).
  4. It sends SIGKILL to what is left.
  5. It journals `stopped` and exits.

The decision engine (design 2):

- The driver keeps one ordered queue of events.
- Each event is journaled, then stepped through the reducers it is routed to (16.9).
- Each effect a reducer returns is journaled, then dispatched (design 7.1).
- Adapters run on their own threads and return observations to the queue.
- Reducers never do IO, and adapters never decide.

Library boundaries:

- Invariant 6: the driver knows the `std.control` types (`Machine`, `Event`, `Effect`,
  `Step`) and the hook names `machine` and `validate`. It knows no domain noun.
- `Restart`, `Availability` and `TelnetPrompt` are library symbols, read only by Ignite code
  (O18 carried over).
- Nothing named after telnet lives under `crates/`. The probe's protocol work is Ignite
  code: `TelnetPrompt.main` over a new telnet client-role fn (16.8).

### 16.3 `ignite run` and `ignite replay`

```
ignite run <file> [--assembly NAME] [--state DIR] [--build DIR] [--target rust|llvm]
                  [--release] [--clock real|manual] [--epoch RFC3339] [--settle DURATION]
                  [--port RESOURCE=PORT]... [--json] [--fresh] [--std PATH] [--fuel N]
ignite replay <file> [--assembly NAME] [--state DIR] [--std PATH] [--fuel N]
```

Steps of `ignite run`:

1. Plan as `ignite plan` does: evaluate, then resolve (E110 to E119). Then check E120 to
   E125 and E129 (16.17).
2. Build every buildable member:
   - the build project lives in `<build>`, default `<state>/build/`;
   - the executables are copied into `<state>/bin/<generation>/`;
   - generation 1 is the start, and a rollout increments it.
3. Open the journal (16.5; E126, E127).
4. Construct the machines and start them.

Paths and builds:

- `<state>` defaults to `target/ignite-run/<assembly>/` under the working directory.
- Builds with unchanged sources are a no-op in `<build>`. Tests pass one shared `--build`
  per target and never rebuild; each test has its own `--state`, because the journal lock
  (E126) admits one run per state directory (A2).

Ports:

- `--port Terminal.ingress=0` binds a free port.
- The driver reads the port from the child's `listening on <address>:<port>` line (6.1) and
  pins it for every restart of that member.

`WorkstationExecutable` members (A10):

- A member whose kind has `main` is built, not started, and the run prints its path.
- A member whose kind has no `main` is listed as "declared, not buildable in slice 12" in
  the `status` output and in the journal's first `segment`. This is not E129. The design
  file's `client: Application` is such a member.

Control input, one command per line on stdin:

| line | effect |
|---|---|
| `status` | prints the current standing of every contract and the state of every machine (one JSON line with `--json`) |
| `rollout <member>` | starts the rollout machine of the member (16.12) |
| `stop` | the same as SIGINT |
| `advance <quantity>` | manual clock only: moves time (16.4); `<quantity>` is a Duration literal such as `100 ms` or `2 seconds` |

Output and exit status:

- One human line per journal record goes to stderr.
- With `--json`, the journal records themselves go to stdout; the tests synchronize on them.
- Exit 0: a requested stop with no failed machine.
- Exit 1: a machine gave up (`gave_up`) or failed (E128).
- Exit 2: a diagnostic before the start.

`ignite replay`:

- It reads the journal of `<state>` and checks the header against the current plan (E126).
- It re-steps the reducers with the journaled events and compares every effect and standing
  record (E127).
- It then prints the final standings.

### 16.4 Clock and time control

The driver owns the only clock the reducers see. Every event carries `at`, a `Duration` since
the journal's epoch. Reducers read no clock (actors R8).

`--clock real` (the default):

- `at` comes from the monotonic clock, offset by the segment's start.
- The epoch is the wall time at which the journal was created.
- A resumed segment continues from the wall time at which it starts (16.5).

`--clock manual`:

- Time moves only on `advance`.
- The epoch is `--epoch`, default `2026-01-01T00:00:00Z`.
- `advance d` fires every due timer in `(at, seq)` order, setting `now` to each timer's
  instant. Then it sets `now` to the target.
- The settle rule: at each instant, before moving on, the driver waits in real time until
  every in-flight adapter operation has completed, at most `--settle` (2 s by default;
  tests pass `--settle 1 s`, A2).
- An operation still running after the settle wait stays in flight. It completes at a later
  instant or at its deadline.
- A healthy probe completes at its dispatch instant, so its latency in manual time is 0.
- A hung probe (its target is SIGSTOPped) costs one settle wait, then times out when time
  reaches its deadline.

Other time rules:

- Calendar periods read the epoch plus `at` as UTC. `std/time.ignite` converts that to civil
  time in the zone (O36).
- Inline Ignite tests of the reducers use no clock: they step synthetic event lists with
  chosen `at` values.

### 16.5 The journal

- Location: `<state>/journal.jsonl`.
- `<state>/lock` is held for the run's lifetime; E126 when another run holds it.
- One JSON object per line, keys in byte order, values in the model encoding of 15.4, and
  `\n` after every record.

Line 1 is the header:

```
{"epoch":{"ns":…,"type":"Duration"},"formatVersion":"ignite-journal/0.1","kind":"header","model":{…the full model…},"seq":0}
```

Every other record has this shape:

```
{"at":{"ns":…,"type":"Duration"},"kind":"<kind>","machine":"<machine id or null>","seq":<n>,"value":<encoded std.control value>}
```

| kind | value | written when |
|---|---|---|
| `segment` | `std.control.Segment` (start, stop, clock mode, generation, declared members that are not buildable) | at run start, resume and stop |
| `event` | `std.control.Event` | before the reducers step it |
| `effect` | `std.control.Effect` | after the step and before dispatch, with `sync_data` before dispatch (O30) |
| `signal` | `{"ref": …, "value": …}`, an emission routed to subscribers (16.9) | when a reducer emits |
| `standing` | `std.control.Standing` (16.10) | at every evaluation |
| `consequence` | `std.control.Fired` (16.11) | at a period close with `violated` |
| `error` | the 14.8 report text plus the code (E128) | at a reducer failure |

Rules:

- `seq` is gap-free from 0.
- A run on an existing journal with the same header model and version appends a `segment`
  record (O31).
- `--fresh` first renames the old file to `journal.<n>.jsonl`.
- A final line without `\n`, from a crash while writing, is dropped on open and reported once
  on stderr. Any other unparsable line is E127.
- There are no snapshots. Resume and replay re-step from `seq` 1 (O30).

### 16.6 Machines: the reducer interface (`std/control.ignite`)

```
enum Machine { Supervise(Supervisor), Schedule(ProbeScheduler), Evaluate(Evaluator), Roll(Rollout) }
type Step { machine: Machine, effects: List<Effect> }
fn step(machine: Machine, event: Event) -> Step
```

`Event`:

- process lifecycle: `Started(pid)`, `Ready`, `Exited(code)`, `Signalled(signal)`,
  `SpawnFailed(reason)`;
- probe: `AttemptDone(id, value)`, `AttemptFailed(id, reason)`;
- `Timer(id)`;
- `Emitted(signal, value)`;
- build: `Built(generation)`, `BuildFailed(reason)`;
- `Released`;
- control: `Control(command)`.

`Effect`:

- process: `Spawn(member, generation)`, `Kill(member, signal)`;
- probe: `Attempt(member, id)`;
- timers: `SetTimer(id, at)`, `CancelTimer(id)`;
- outputs: `Emit(signal, value)`, `Publish(Standing)`, `Fire(Fired)`;
- `Build(member)`;
- `AwaitRelease(member)`;
- `Note(text)`, a journal line with no adapter.

Payloads carry `at` where the reducer needs it.

Construction:

- The evaluator calls `K.machine(self, id: Text) -> Machine` for every member and contract
  the assembly hosts whose kind defines `machine`.
- This happens at plan time, so the initial machines are part of the model (16.14).
- Machine ids are `<member path>.<role>`: `Terminal.server.supervisor`,
  `Terminal.promptProbe.scheduler`, `Terminal.reachable.evaluator`,
  `Terminal.server.rollout`.
- `Machine` is a closed std enum in slice 12. A user kind cannot define a new machine (O39).

Failures:

- A reducer failure (assert, overflow, fuel per step O14) ends that machine only (T1).
- The run journals `error` with E128 and continues. The exit status becomes 1.

### 16.7 Supervisor and `Restart`

`TelnetServer.machine` returns `Supervise` with the member's `recovery: Restart`. The states
follow design 11.8: `Starting`, `Running`, `Backoff`, `Failed`, `Draining`, `Stopped`. The
supervisor also does the work of design 11.8's listener lifecycle (O44).

Readiness:

- `Ready` is the adapter's event for the child's `listening on` line. Generated `main` binds
  the listener before `K.main` (6.1).
- A member with no listener is ready at `Started`.
- With no `Ready` before `readiness_timeout`, the supervisor sends `Kill` and counts a
  failure.
- The new field is `Restart.readiness_timeout: Duration = 10 seconds` (O37).

Failures:

- An exit or signal that no `Kill` of this machine requested is a failure, exit 0 included,
  because a server does not end by itself.
- The attempt number `n` is the number of failures within the trailing `interval`, plus 1.
- If `allows(restart, n)`: the state becomes `Backoff`, with
  `SetTimer(at + delay(backoff, n))`. This is journaled as the effect `restart` with
  `attempt` and `delay`. At the timer comes `Spawn`.
- Otherwise: the state becomes `Failed`, with `Note("gave_up")`. Other machines go on (O38).
- `delay` and `allows` are the existing std fns, and their tests stay.

### 16.8 Probe scheduler and the probe process

The probe executable:

- `TelnetPrompt` gains `executable: Text` (defaulting to the component name, the 6.1 rule)
  and `fn TelnetPrompt.main(self) -> IoResult<Unit>`.
- `ignite build` emits it as an executable like any kind with `main`. This changes builds of
  the design file only.

`main` runs one attempt:

1. Resolve `self.target` with `Endpoint.of` (16.9).
2. Connect.
3. Read through `Telnet.await_prompt(stream, expected_prompt)`. This is a new client-role fn
   in `std/protocol/telnet.ignite`: it answers option negotiation as `connect` does and
   returns the text before the prompt.
4. Measure `latency` with `Clock.monotonic()`.
5. Close if `close_after_probe`.
6. Print one line, the 15.4 encoding of `std.probe.ProbeAttempt`.
7. Exit 0.

A refused or reset connection gives `received = false`, a bad attempt.

The scheduler: `TelnetPrompt.machine` returns `Schedule`.

- Routing of lifecycle events (A4): the driver sends a member's lifecycle events
  (`Started`, `Ready`, `Exited`, `Signalled`, `SpawnFailed`) to its supervisor. It also sends
  them to every scheduler that has an `edges.readiness` edge to that member (16.14).
  Readiness is no signal and does not appear in `observations`.
- Slots: one every `interval`, from the target's first `Ready` on. Earlier slots are outside
  the evidence horizon, not missing (O40).
- At each slot the scheduler returns `Attempt(member, id)` and
  `SetTimer(deadline = at + timeout)`.
- The adapter spawns the probe process (through the spawner thread, 16.2) and decodes its
  stdout line by the signal's element type.
- `AttemptDone` before the deadline gives `Emit(attempts, value)`.
- The deadline first gives `Kill` of the probe process and
  `Emit(attempts, ProbeAttempt{received: false, latency: timeout})`. This is journaled as the
  effect `probe timeout`, a bad event (11.7).
- `AttemptFailed` gives no emission and a `Note("missing")`. Its causes: no executable, an
  exit status other than 0, a line that does not decode. The slot counts as expected and not
  executed.
- There is one probe process per attempt (O28). Probes of one member never overlap, because
  E123 guarantees `timeout < interval`.

### 16.9 Signals and references at run time

Signals:

- A `Signal<T>` value is a stream name, its 15.4 `ref` text.
- Only reducers emit, through `Effect.Emit`.
- The driver journals the emission as a `signal` record. It routes it as `Event.Emitted` to
  every machine whose model entry lists that ref in `subscribes`, in machine-id byte order.
- Lifecycle events are routed by 16.8's rule. Every other event goes to the machine it names.
- Compiled programs do not emit signals in slice 12. A probe's stdout line is an adapter
  result, not an emission.
- Pub/sub between processes is wave C (actors.md).

References:

- A `Ref<K>` value in a compiled program is its reference text (6.1).
- The new builtin `Endpoint.of(ref) -> IoResult<Endpoint>` resolves it through the process
  arguments `--ref <path>=<address>:<port>` that the driver passes.
- Outside `ignite run` it fails with "no binding for <path>". This mechanism is new for
  references, and works like the existing `--port`.
- The driver resolves a component ref to the endpoint of the single `TcpListener` resource
  with a `provisioning` edge to that component (16.14). It applies `--port` overrides and
  pinned ports first.
- No such resource, or several, is E122.

### 16.10 Contract evaluation

- `Availability.machine`, `ThresholdCompliance.machine` and `SLA.machine` return `Evaluate`.
  Each subscribes to its `signal`.
- A good event satisfies `satisfies(attempt, good, threshold)` (std/probe). `threshold` is 0
  for `Availability` and `SLA`.
- Population: the executed attempts in the window (11.7).
- Missing: the expected slots in the window that are not executed.

The `Standing` record:

- `contract`;
- `status`: `Compliant`, `Violated` or `InsufficientEvidence`;
- `good`, `executed`, `expected`;
- `completeness` (a Ratio);
- `from`, `to`;
- `coverage`: the Ratio of the window inside the evidence horizon;
- `calculation = "ignite-sli/0.1"`;
- `version`: the record's `seq`.

The status, decided in this order:

1. Apply `missing_data`: `counts_as_good` and `counts_as_bad` fill the missing slots.
2. `executed == 0` gives `InsufficientEvidence`.
3. `completeness < minimum_completeness`, when `missing_data` is `insufficient_evidence`,
   gives `InsufficientEvidence`.
4. `meets(target, good, executed)` gives `Compliant`; otherwise `Violated`.

Std changes:

- The three kinds gain the field `minimum_completeness: Ratio = 90%` (O32).
- `meets` changes: `total == 0` now gives `false`.
- Its inline test `meets(100%, 0, 0)` flips. The flipped test cites design 4.1: "Zero traffic
  does not automatically prove compliance" (A15).
- The new fn `standing(...)` is the evaluation entry.

Windows:

- `Rolling(d)` keeps counts in buckets of `max(signal interval, d / 10,000)` (O33). A 30-day
  window has 10,000 buckets.
- `CalendarMonth(zone)` keeps the current period's counts. The evaluator sets a timer at the
  next month boundary in `zone`.
- Evaluation runs on every emission and at every period boundary.
- Fuel (A7): an inline test steps an `Evaluate` machine with a 30-day `Rolling` window and
  `interval = 5 seconds` through 10,000 emissions in the evaluator, under the default fuel
  per step (O14). Item 12 records its time.
- If the fuel does not hold, O33 becomes `window / 1,000`, and that test is the evidence.

### 16.11 Consequences: `event("…")`

At an `SLA` period boundary, the evaluator publishes the closed period's standing.

- If the standing is `Violated`, the evaluator returns `Fire(Fired{contract, consequence,
  period_from, period_to, standing_seq})`.
- The journal record is the consequence's dispatch. Slice 12 has no external sink. Design 7.1
  requires the intent to be persisted; an adapter for delivery comes later.
- There is one `Fired` per contract and period, and replay and resume never fire it twice.
  The key is `(contract, period_from)`, checked against the journal.
- `InsufficientEvidence` fires nothing. It journals `Note("consequence withheld: insufficient
  evidence")` (invariant 5: no silent loss).
- `exclusions` must be empty in slice 12; anything else is E120. They are not evaluated, so
  accepting them would ignore them without a word.

### 16.12 `DrainRollout`

Design 11.8: "Until it does, planning rejects that rollout binding or requires an explicit
stop-and-start strategy with declared disruption. It must not simulate zero-downtime
behavior in its plan." Slice 12 declares the disruption (O34).

The declaration:

- The model's rollout machine entry carries `"disruption": "connections refused between
  listener close and rebind"`.
- `ignite explain Terminal.server.rollout` prints it.

The sequence, triggered by `rollout <member>`:

1. `Build` writes generation n+1 (a no-op build when the sources are unchanged, 16.3).
2. `Kill(old, SIGTERM)`: the old process drains (16.13).
3. `AwaitRelease`: the adapter reads the old process's stdout line `released
   <address>:<port>`. The drain prints it after the listener is closed, the `listening on`
   rule in reverse (A5). The adapter then sends `Released`. It does not connect-poll.
4. `Spawn(new)`, then await `Ready`.
5. The old process exits when its sessions end, and the journal records `retired`. If it
   still runs at `deadline`, `on_deadline` applies (`DisconnectRemaining` sends SIGKILL),
   and the journal records `retired forced`.

Rules:

- The journal records `disruption` with the measured interval from `Released.at` to
  `Ready.at`.
- The old process's exit during `Draining` is not a failure. A failure of the new process
  follows 16.7.
- `on_deadline` stays `Text`, with the closed set `{"disconnect_remaining"}` (O35).
  `fn on_deadline(text) -> Option<OnDeadline>` parses it, as `missing_data` does; any other
  value is E120.
- `BuildFailed` aborts the rollout before step 2, and the old process keeps serving.

### 16.13 Substrate additions (`crates/ignite-io`, both targets)

| builtin | behaviour | test |
|---|---|---|
| `Clock.monotonic() -> Duration` | the time since an arbitrary start of the process; it never decreases | inline: two reads are ordered |
| `Endpoint.of(ref: Ref<K>) -> IoResult<Endpoint>` | the `--ref path=address:port` argument for `ref`'s text; otherwise the IoError "no binding for <path>" | inline (no binding) and e2e (probe) |
| drain on SIGTERM | see the rules below | e2e: an open session survives SIGTERM; a new connect is refused after `released` |

The drain on SIGTERM:

- A signal thread (`sigwait`, with SIGTERM blocked in all other threads) sets drain mode.
- It shuts down the read side of every listener and prints `released <address>:<port>` for
  each one once it is closed (A5).
- A pending `accept` fails with "draining".
- After `K.main` returns, `run` waits for the live tasks to end, then exits 0.
- A second SIGTERM exits at once.
- Outside `ignite run`, the wait for live tasks has no bound. Only the driver's deadline
  bounds it (A6).
- Existing tests end servers with `Child::kill` (SIGKILL), so the SIGTERM change touches none
  of them.

Other rules:

- The substrate has no process spawning and no journal API. Both are driver code; a compiled
  program does neither.
- Lowering: rustgen calls the builtins directly; ignite-llvm calls them through C ABI entries
  in `ignite-rt`. The type checker knows their signatures.
- Builds of programs that use none of them stay byte-identical (16.0 item 11).

### 16.14 Model additions (additive, `ignite-model/0.1`)

| key | content |
|---|---|
| `machines` | array sorted by id: `{"id", "machine": <encoded initial Machine value>, "member", "subscribes": [refs], "disruption"?}` |
| `observations` | array sorted by ref: `{"ref", "producer", "element": "<qualified type>", "interval"}` |
| `edges.provisioning` | resource → the component that binds it (`Terminal.ingress` → `Terminal.server`) |
| `edges.readiness` | probe → its target; evaluator → its signal's producer; the routing of 16.8 reads it |
| `edges.recovery` | member → its supervisor machine |

- O25's deferred edge classes land here.
- `ignite explain` accepts machine ids as paths. It prints the machine's state fields and its
  `disruption`.

### 16.15 Replay

- `ignite replay` and the end-to-end tests share one fn. It takes the model and the journal's
  `event` records and re-steps the machines. It produces the `effect`, `signal`, `standing`
  and `consequence` records.
- They must equal the journaled ones byte for byte. The first difference is E127, with its
  `seq`.
- The values a real process contributes (`pid`, `latency`, `code`) enter only through `event`
  records, so replay is exact.
- Design 7 is the source: "Interpreter and native execution share semantic tests and replay
  traces" (A13).

### 16.16 Typed API (add-only)

- `ignite_eval`:
  - `Value::decode(json: &str, ty: &Type) -> Result<Value, DecodeError>`, the inverse of
    15.4;
  - `Evaluator::call(&mut self, fn path, args) -> Result<Value, EvalError>`;
  - `Model.machines` and `Model.observations`.
- `ignite`:
  - `RunOptions{assembly, state, build, target, release, clock, epoch, settle, ports, json,
    fresh, std, fuel}`;
  - `run(&Path, &RunOptions) -> Result<i32, Diagnostics>`;
  - `replay(&Path, &ReplayOptions) -> Result<Vec<Standing>, Diagnostics>`.
- `ignite-types`: the signatures of the three builtins of 16.13.

### 16.17 Diagnostics E120 to E129

| code | rule | phase | fixture |
|---|---|---|---|
| E120 | a declaration its kind's `validate` refuses, with the reason text from std: an unknown `good` outcome, `missing_data`, `on_deadline` or time zone; `maximum_attempts` < 1; `target` not in (0 %, 100 %]; non-empty `exclusions`; a backoff initial greater than its maximum | check | `e120_validate_refused` |
| E121 | a contract's `signal` has no producer the assembly runs: its member is not bound to a host that runs it, or the ref is no output | check | `e121_signal_without_producer` |
| E122 | a run-time `Ref` (a probe's `target`) names a member the assembly does not run as a process, or resolves to no endpoint or to several | check | `e122_unresolvable_ref` |
| E123 | probe timing: `interval` or `timeout` not positive, or `timeout >= interval` | check | `e123_probe_timing` |
| E124 | a contract window shorter than its signal's `interval`, so it can hold no expected attempt | check | `e124_window_too_short` |
| E125 | `ignite run` on a file without an assembly, or on an assembly that runs nothing (no member with a machine) | run | `e125_nothing_to_run` |
| E126 | the state directory's journal has another model or `formatVersion`, or another `ignite run` holds its lock | run, replay | `e126_journal_mismatch` |
| E127 | a journal that does not replay: an unparsable line before the last, or a re-derived record that differs (first `seq`) | run (resume), replay | `e127_replay_diverged` |
| E128 | a reducer failed at run time (14.8 report text; fuel per step); the machine stops, and the run continues and exits 1 | run | `e128_reducer_failed` (a run with `--fuel 1`) |
| E129 | a member the assembly hosts whose kind lacks the entry its host needs: `ScheduledProbeHost` without `K.main` or `K.machine`, `NativeProcess` without `K.machine` | check | `e129_missing_entry` |

- Phase "check": `ignite check` and `ignite plan` report these codes for every assembly (O20
  carried over).
- `ignite check` of the design file still exits 0 with no output, which
  `cli::check_design_file_is_silent` asserts (A14).
- `validate` is a hook like `machine`: `fn K.validate(self) -> List<Text>`, evaluated purely.
  The compiler knows no outcome names and no zone names (invariant 6).
- Fixtures live in `crates/ignite/tests/fixtures/`.
- E129 is the last code of this slice (T4).

### 16.18 Work split

| agent | owns |
|---|---|
| P0 | none (16.1) |
| P1 frontend and skeleton | `ignite-syntax`, `ignite-check`, `ignite-types` (the builtin signatures of 16.13, the hook signature checks); `crates/ignite-eval/src/resolve.rs` and the new `validate.rs` (E120 to E124, E129); `std/control.ignite` type declarations only (`Machine`, `Event`, `Effect`, `Step`, `Standing`, `Fired`, `Segment`), written first, then handed to P4; the `run` and `replay` CLI stubs and the `crates/ignite/src/run/` module stubs with their placeholders (16.20); `docs/slice.md` for the whole slice |
| P2 substrate and adapters | `crates/ignite-io` (16.13), the `crates/ignite-rt` entries, and the lowering of the three builtins in `ignite-rustgen` and `ignite-llvm`; `crates/ignite/src/run/{process,probe,clock,journal,control}.rs` (the adapters with the spawner thread, the manual clock with the settle rule, the journal file and lock); substrate tests in `crates/ignite-io/tests/` |
| P3 driver loop, model, replay, tests | `crates/ignite-eval/src/{model,value,eval,explain}.rs` (16.14, `decode`, `call`); `crates/ignite/src/run/{mod,engine,replay}.rs`, `crates/ignite/src/lib.rs` and `main.rs`; `crates/ignite/tests/**`, including `main.rs` and `common/mod.rs`; E125 to E128; the model and explain goldens; the acceptance tests of items 1 to 3 and 5 to 9 |
| P4 std, examples, records, acceptance | the `std/control/*.ignite` reducers with inline tests (the A7 fuel test among them), `std/time.ignite`, `std/contract.ignite`, `std/policy.ignite`, `std/probe.ignite` (`main`, `machine`, `validate`), `std/protocol/telnet.ignite` (`await_prompt`), `std/net.ignite` (the `Endpoint.of` declaration); the reducer trace fixture of item 10 (the file goes in `crates/ignite/tests/fixtures/`; its test comes by message to P3); `examples/**`; `AGENTS.md` (layout rows, the slice-12 gate-time row); `docs/friction.md`; design notes in 10.1 (`run` and `replay` exist) and 11.8; acceptance items 4 and 11 to 13, verified with commands |

Order:

- P1 comes first: the skeleton and the `std/control.ignite` types.
- Then P2, P3 and P4 run in parallel. P4 starts with `std/control` and `std/probe`, because
  P3's engine steps them.
- 16.12 (`DrainRollout`) and the drain row of 16.13 are the last units of P2 and P4. They
  start after acceptance items 1 to 5 and 7 to 10 pass (A12).
- If the slice runs long, the close reports the rollout as the carried item, and the rest
  closes. This is no cut: vision row 12 keeps `DrainRollout` in the slice.
- P4's acceptance comes last.

Shared files:

- `crates/ignite/tests/main.rs` and `tests/common/mod.rs` are P3's. P2 and P4 ask P3 for
  their `mod` lines.
- After the skeleton, `std/control.ignite` changes only by message between P3 and P4.

Rules for tests and dependencies:

- One integration-test binary per crate: new files are modules of the existing `main.rs` of
  `crates/ignite`, `crates/ignite-io` and `crates/ignite-eval`. No `[[test]]` entry is added.
- `nix` moves from a dev-dependency to a dependency of `crates/ignite`, for pdeathsig,
  process groups and signals (O43). It is already a workspace dependency of `ignite-io`, so
  no new crate is added.

### 16.19 Decisions taken in the review

| # | decision |
|---|---|
| O27 | Reducers run in the evaluator inside `ignite run`. The trace fixture of item 10 also compiles them on both targets. Steering amended vision 4.1 to say so (A1). Wave C revisits a compiled controller with actors |
| O28 | One probe process per attempt. `latency` is the probe's own connect-to-prompt time. A resident probe host is a later milestone |
| O29 | Tests control time with the manual clock, driven by `advance` on stdin, and the settle rule of 16.4. The default is `--settle 2 s`; tests pass `--settle 1 s` (A2) |
| O30 | `sync_data` after every `effect` record, before dispatch. No snapshots; resume re-steps from `seq` 1 |
| O31 | A run appends to a journal with the same model. The gap between segments is missing evidence. `--fresh` renames the old journal |
| O32 | A new field `minimum_completeness: Ratio = 90%` on `Availability`, `ThresholdCompliance` and `SLA`. Zero executed attempts give `insufficient_evidence`, and `meets(_, 0, 0)` becomes `false`. Source: design 4.1, "Zero traffic does not automatically prove compliance" (A15) |
| O33 | Buckets of `max(interval, window / 10,000)`, calculation version `ignite-sli/0.1`. They become `window / 1,000` if the fuel test of 16.10 fails (A7) |
| O34 | `DrainRollout` without a handoff is declared by the plan (`machines[].disruption`). No user field is added, and the rollout is not rejected. Design 11.8: "Until it does, planning rejects that rollout binding or requires an explicit stop-and-start strategy with declared disruption." (A9) |
| O35 | `on_deadline` stays `Text`, with a closed set checked by `validate`. This reverses the std comment that announced an enum, and the design file stays unchanged |
| O36 | Time zones are a closed set in `std/time.ignite`: `UTC` and `Europe/Berlin` (the EU DST rule, in Ignite). Any other zone is E120 |
| O37 | Readiness is the child's `listening on` line. `Restart.readiness_timeout = 10 seconds`; a timeout counts as a failure |
| O38 | A member with exhausted restarts stays `Failed`. The run continues and exits 1 at stop |
| O39 | No user-defined machines in slice 12: `Machine` is a closed std enum |
| O40 | Expected slots start at the target's first `Ready`. Partial windows are evaluated and report `coverage` |
| O41 | All `ignite run` end-to-end tests together add at most 6 s of wall time to `crates/ignite/tests`. Only the hung-probe scenario (one test, one target) pays a settle wait. Tests build the design file once per target into one shared `--build` and never rebuild. They never use port 2323 (A2) |
| O42 | `TelnetPrompt` gains `executable` (defaulting to the component name) and `main`, so the design file builds a `promptProbe` binary |
| O43 | `nix` becomes a normal dependency of `crates/ignite`; no new crate |
| O44 | Design 11.8 lists five machines, and slice 12 has four (A8). It folds the listener lifecycle into the supervisor: `listening on` gives `Ready`, and the `released` line gives `Released`. The design row is "Listener lifecycle: Observe, bind or adopt, report ready, release according to ownership". Adopt and bind ownership are slice 13's |

### 16.20 Placeholder arms

P1's skeleton makes the workspace build with `--all-targets`. Each placeholder is
`unimplemented!("slice 12: …")`, and its owner replaces it:

| owner | file | fn | covers |
|---|---|---|---|
| P2 | `ignite-io/src/clock.rs` | `monotonic` | 16.13 |
| P2 | `ignite-io/src/net.rs` | `endpoint_of` | 16.9, 16.13 |
| P2 | `ignite-io/src/lib.rs` | `drain` (signal thread) | 16.13 |
| P2 | `ignite/src/run/process.rs` | `spawn`, `kill` | 16.2, 16.7 |
| P2 | `ignite/src/run/probe.rs` | `attempt` | 16.8 |
| P2 | `ignite/src/run/clock.rs` | `Manual::advance` | 16.4 |
| P2 | `ignite/src/run/journal.rs` | `Journal::open`, `Journal::append` | 16.5 |
| P3 | `ignite/src/run/engine.rs` | `Engine::step` | 16.2, 16.6 |
| P3 | `ignite/src/run/replay.rs` | `replay` | 16.15 |
| P3 | `ignite/src/lib.rs` | `run`, `replay` | 16.3 |
| P3 | `ignite-eval/src/value.rs` | `Value::decode` | 16.16 |
| P3 | `ignite-eval/src/model.rs` | `machines`, `observations` | 16.14 |
| P1 | `ignite-eval/src/validate.rs` | `validate` | 16.17 |

- That is 17 placeholders in 13 rows.
- The std fns have no placeholders: P4 writes them on top of P1's type declarations.

### 16.21 Implementation record

Empty until the slice closes.
