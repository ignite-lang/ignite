# std.contract

Contracts over a probe's attempts: what fraction of attempts must be good, over which window,
and what happens to attempts the probe never made. A contract is a declaration inside a
`system` (`contract reachable: Availability { ... }`). `ignite check` validates it, and
`ignite run` evaluates it over the live signal with the `Evaluate` machine of
[std.control](control.md).

- Layer: general.
- Source: [`std/contract.ignite`](https://github.com/ignite-lang/ignite/blob/main/std/contract.ignite).
- Import: `import std.contract.{Availability, ThresholdCompliance, SLA, event}`.

The window, the missing-data rule and the compliance arithmetic come from
[std.sli](sli.md): `good` is an outcome name such as `prompt_received_before_timeout`,
`window` is `rolling(span)` or `calendar_month(zone)`, and `missing_data` is
`insufficient_evidence`, `counts_as_good` or `counts_as_bad`.

## Contract kinds

```ignite,fragment
type Availability {
  signal: Signal<ProbeAttempt>
  good: Text
  target: Ratio
  window: Window
  missing_data: Text = "insufficient_evidence"
  minimum_completeness: Ratio = 90%
}

type ThresholdCompliance {
  signal: Signal<ProbeAttempt>
  good: Text
  threshold: Duration
  target: Ratio
  window: Window
  missing_data: Text = "insufficient_evidence"
  minimum_completeness: Ratio = 90%
}

type SLA {
  signal: Signal<ProbeAttempt>
  good: Text
  target: Ratio
  period: Window
  exclusions: List<Text> = []
  consequence: Consequence
  minimum_completeness: Ratio = 90%
}
```

| kind | the contract |
|---|---|
| `Availability` | at least `target` of the attempts in `window` are `good` |
| `ThresholdCompliance` | at least `target` of the attempts in `window` are `good` within `threshold` |
| `SLA` | a commitment: `target` of the attempts in each `period` are `good`, else `consequence` |

- Evidence covering less than `minimum_completeness` of the expected attempts is
  insufficient.
- For an `SLA`, missing attempts are always insufficient evidence. `exclusions` (named
  maintenance windows) are not evaluated yet, so none may be declared.

Each kind has two hooks:

```ignite,fragment
fn Availability.machine(self: Availability, id: Text) -> Machine
fn Availability.validate(self: Availability) -> List<Text>
fn ThresholdCompliance.machine(self: ThresholdCompliance, id: Text) -> Machine
fn ThresholdCompliance.validate(self: ThresholdCompliance) -> List<Text>
fn SLA.machine(self: SLA, id: Text) -> Machine
fn SLA.validate(self: SLA) -> List<Text>
```

- `machine` returns the contract's evaluator, `Machine.Evaluate`, with the id `<id>.evaluator`.
  An `SLA`'s evaluator fires the consequence at a period close whose standing is `Violated`.
- `validate` returns the reasons `ignite check` reports as E120: an unknown `good` outcome, a
  `target` outside (0 %, 100 %], an unknown time zone, a window that is not positive, an
  unknown `missing_data`, a `minimum_completeness` above 100 %; for `ThresholdCompliance` a
  `threshold` that is not positive; for an `SLA` any `exclusions`.

## Consequences

```ignite,fragment
enum Consequence {
  Event(Text)
}

fn event(name: Text) -> Consequence
fn consequence_name(consequence: Consequence) -> Text
```

What a breached `SLA` causes: `event("access_commitment_breached")` raises the named event.
Under `ignite run` the event is recorded in the run's journal; there is no external delivery
yet.

## Helpers

```ignite,fragment
fn reasons(good: Text, target: Ratio, window: Window, missing: Option<Text>, minimum_completeness: Ratio) -> List<Text>
fn missing_rule(text: Text) -> MissingData
```

| function | what it does |
|---|---|
| `reasons` | the declaration rules the three kinds share, as reason texts |
| `missing_rule` | the missing-data rule of a declaration's text; `InsufficientEvidence` for a text `validate` would refuse |

## In a declaration

From [`terminal.design.ignite`](https://github.com/ignite-lang/ignite/blob/main/examples/terminal/terminal.design.ignite),
where `promptProbe` is a [`TelnetPrompt`](telnet.md):

```ignite,fragment
contract reachable: Availability {
  signal = promptProbe.attempts
  good = "prompt_received_before_timeout"
  target = 99.9%
  window = rolling(30 days)
  missing_data = "insufficient_evidence"
}

contract promptFast: ThresholdCompliance {
  signal = promptProbe.attempts
  good = "prompt_received_within_threshold"
  threshold = 200 ms
  target = 99%
  window = rolling(1 hour)
  missing_data = "insufficient_evidence"
}

contract customerAccess: SLA {
  signal = promptProbe.attempts
  good = "prompt_received_before_timeout"
  target = 99.5%
  period = calendar_month("Europe/Berlin")
  exclusions = []
  consequence = event("access_commitment_breached")
}
```

## Example

```ignite
import std.contract.*
import std.sli.{Window, MissingData}

test "consequences are built by name" {
  assert consequence_name(event("access_commitment_breached")) == "access_commitment_breached"
}

test "the shared declaration rules" {
  assert reasons("prompt_received_before_timeout", 99.9%, Rolling(30 days), Some("insufficient_evidence"), 90%) == []
  assert reasons("prompt_received_before_timeout", 99%, CalendarMonth("Europe/Paris"), None, 90%) == ["unknown time zone \"Europe/Paris\": the closed set is UTC, Europe/Berlin"]
  assert reasons("prompt_received_before_timeout", 0%, Rolling(1 hour), None, 90%) == ["target 0% is not in (0 %, 100 %]"]
  assert missing_rule("counts_as_good") == CountsAsGood
  assert missing_rule("nonsense") == MissingData.InsufficientEvidence
}
```

How a standing is computed from the counts (the status order, completeness, coverage) is
described in [std.control](control.md#evaluator).
