# std.sli

The vocabulary of service-level indicators: the sample a probe emits, the outcome a contract
names, the window a contract is judged over, the rule for attempts the probe never made, and
the target comparison. [std.contract](contract.md), the Telnet probe and the reducers of
[std.control](control.md) all read it. It imports nothing, so no import cycle can arise.

- Layer: leaf.
- Source: [`std/sli.ignite`](https://github.com/ignite-lang/ignite/blob/main/std/sli.ignite).
- Import: `import std.sli.{ProbeAttempt, Window, satisfies, meets}`.

## Probe attempts and outcomes

```ignite,fragment
type ProbeAttempt {
  received: Bool
  latency: Duration
}

fn satisfies(attempt: ProbeAttempt, good: Text, threshold: Duration) -> Bool
fn known_outcome(good: Text) -> Bool
```

- `ProbeAttempt` is one attempt of a prompt probe: whether the prompt arrived before the
  probe's timeout, and how long it took. It is the element type of a probe's
  `Signal<ProbeAttempt>`.
- `satisfies` tells whether an attempt counts as good for the outcome a contract names in
  `good`:

| outcome | holds when |
|---|---|
| `prompt_received_before_timeout` | the prompt arrived |
| `prompt_received_within_threshold` | the prompt arrived in at most `threshold` |
| any other name | never |

- `known_outcome` is whether `good` names one of these two outcomes. The contract kinds'
  `validate` refuses any other name (E120).

## Windows

```ignite,fragment
enum Window {
  Rolling(Duration),
  CalendarMonth(Text)
}

fn rolling(span: Duration) -> Window
fn calendar_month(time_zone: Text) -> Window
```

| window | span |
|---|---|
| `rolling(span)` | the last `span` of time, ending now |
| `calendar_month(time_zone)` | whole calendar months in the IANA time zone `time_zone`; [std.time](time.md) knows `UTC` and `Europe/Berlin` |

## Missing data

```ignite,fragment
enum MissingData {
  InsufficientEvidence,
  CountsAsGood,
  CountsAsBad
}

fn missing_data(text: Text) -> Option<MissingData>
```

How attempts the probe never made count. A declaration writes it as text
(`missing_data = "insufficient_evidence"`); `missing_data` reads the closed set
`insufficient_evidence`, `counts_as_good` and `counts_as_bad`, and gives `None` for anything
else.

## The target check

```ignite,fragment
fn meets(target: Ratio, good: Int, total: Int) -> Bool
```

Whether `good` of `total` attempts reach `target`, compared in parts per million. No attempts
reach no target: zero traffic does not prove compliance.

## Example

```ignite
import std.sli.*

test "an attempt satisfies the outcomes it earned" {
  let fast = ProbeAttempt { received = true, latency = 50 ms }
  let slow = ProbeAttempt { received = true, latency = 900 ms }
  let missed = ProbeAttempt { received = false, latency = 2 seconds }
  assert satisfies(fast, "prompt_received_within_threshold", 200 ms)
  assert satisfies(slow, "prompt_received_before_timeout", 200 ms)
  assert !satisfies(slow, "prompt_received_within_threshold", 200 ms)
  assert !satisfies(missed, "prompt_received_before_timeout", 200 ms)
  assert !known_outcome("unknown")
}

test "windows, missing data and targets" {
  assert rolling(30 days) == Rolling(30 days)
  assert calendar_month("Europe/Berlin") == CalendarMonth("Europe/Berlin")
  assert missing_data("counts_as_bad") == Some(CountsAsBad)
  assert missing_data("Insufficient_Evidence") == None
  assert meets(99.9%, 999, 1000)
  assert !meets(99.9%, 998, 1000)
  assert !meets(100%, 0, 0)
}
```
