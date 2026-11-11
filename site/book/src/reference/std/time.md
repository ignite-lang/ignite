# std.time

Civil time for calendar windows. An instant is a `Duration` since 1970-01-01T00:00:00Z, as
`Clock.now()` of [std.io](io.md#clock) reads it. The
module finds the start of the calendar month that contains an instant, and the next one, in a
time zone. The evaluator of [std.control](control.md) uses it for the period boundaries of a
`CalendarMonth` window.

- Layer: leaf.
- Source: [`std/time.ignite`](https://github.com/ignite-lang/ignite/blob/main/std/time.ignite).
- Import: `import std.time.{month_start, next_month_start, zone_known}`.

Zones are a closed set: `UTC` and `Europe/Berlin`. `Europe/Berlin` follows the EU rule:
summer time from the last Sunday of March 01:00 UTC to the last Sunday of October 01:00 UTC.
Any other zone is unknown, and the contract kinds' `validate` refuses it (E120).

## Month boundaries

```ignite,fragment
fn zone_known(zone: Text) -> Bool
fn month_start(zone: Text, instant: Duration) -> Option<Duration>
fn next_month_start(zone: Text, instant: Duration) -> Option<Duration>
```

| function | what it does |
|---|---|
| `zone_known` | whether `zone` is `UTC` or `Europe/Berlin` |
| `month_start` | the instant at which the calendar month containing `instant` starts in `zone`; `None` for an unknown zone |
| `next_month_start` | the first month start in `zone` strictly after `instant`; `None` for an unknown zone |

A month with a summer time change is an hour longer or shorter: in `Europe/Berlin`, March
2026 is 31 days less one hour and October 2026 is 31 days and one hour.

## Example

```ignite
import std.time.*

test "month boundaries in UTC and Europe/Berlin" {
  let at = 1793487000 seconds
  assert month_start("UTC", at) == Some(1790812800 seconds)
  assert next_month_start("UTC", at) == Some(1793491200 seconds)
  assert month_start("Europe/Berlin", at) == Some(1790805600 seconds)
  assert next_month_start("Europe/Berlin", at) == Some(1793487600 seconds)
  assert month_start("Mars/Olympus", at) == None
  assert zone_known("UTC") && !zone_known("Europe/Paris")
}
```
