# std.sli

The vocabulary of service-level indicators: the outcome of the prompt probe and its predicates,
the sample a scheduler emits and the verdict an evaluator counts,
the window a contract is judged over, the rule for attempts the probe never made, and
the target comparison. [std.contract](contract.md), the Telnet probe and the reducers of
[std.control](control.md) all read it. It imports nothing, so no import cycle can arise.

- Layer: leaf.
- Source: [`std/sli.ignite`](https://github.com/ignite-lang/ignite/blob/main/std/sli.ignite).
- Import: `import std.sli.{ProbeAttempt, Window, prompt_received_before_timeout, meets}`.

## Probe attempts and outcomes

```ignite,fragment
type ProbeAttempt {
  received: Bool
  latency: Duration
}

fn prompt_received_before_timeout(attempt: ProbeAttempt) -> Bool
fn prompt_received_within_threshold(attempt: ProbeAttempt, threshold: Duration) -> Bool
```

- `ProbeAttempt` is one attempt of a prompt probe: whether the prompt arrived before the
  probe's timeout, and how long it took. It is the outcome type of the Telnet probe's
  `Signal<ProbeAttempt>`.
- The two predicates are what a contract over that signal names in `good`:

| predicate | holds when |
|---|---|
| `prompt_received_before_timeout` | the prompt arrived |
| `prompt_received_within_threshold` | the prompt arrived in at most `threshold` |

A probe kind outside `std` declares its own outcome type and predicates; a contract's `good`
is any named fn over the outcome type of its signal.

## Samples and verdicts

```ignite,fragment
type Sample {
  slot: Duration
  line: Option<Text>
}

type Verdict {
  slot: Duration
  good: Bool
}
```

- `Sample` is one attempt as a probe scheduler emits it: the slot it was dispatched for and the
  probe's printed line, `None` when the attempt timed out.
- `Verdict` is one emission as a contract's evaluator receives it: the slot and whether the
  contract's `classify` held for the decoded line (`false` for a sample without a line).

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

test "the prompt outcomes are predicates" {
  let fast = ProbeAttempt { received = true, latency = 50 ms }
  let slow = ProbeAttempt { received = true, latency = 900 ms }
  let missed = ProbeAttempt { received = false, latency = 2 seconds }
  assert prompt_received_within_threshold(fast, 200 ms)
  assert prompt_received_before_timeout(slow)
  assert !prompt_received_within_threshold(slow, 200 ms)
  assert !prompt_received_before_timeout(missed)
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
