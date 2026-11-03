# std.time

Civil time for calendar windows. An instant is a `Duration` since 1970-01-01T00:00:00Z. The
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

## Calendar arithmetic

The helpers below are exported too. Seconds are UTC seconds since the epoch; days are days
since 1970-01-01.

```ignite,fragment
fn days_from_civil(year: Int, month: Int, day: Int) -> Int
fn civil_from_days(days: Int) -> (Int, Int, Int)
fn last_sunday(year: Int, month: Int) -> Int
fn berlin_summer(utc: Int) -> Bool
fn offset(zone: Text, utc: Int) -> Option<Int>
fn local_midnight_utc(zone: Text, year: Int, month: Int) -> Int
fn month_start_seconds(zone: Text, utc: Int) -> Option<Int>
fn next_month_start_seconds(zone: Text, utc: Int) -> Option<Int>
fn seconds(instant: Duration) -> Int
```

| function | what it does |
|---|---|
| `days_from_civil` | days since 1970-01-01 of a proleptic Gregorian date |
| `civil_from_days` | the date `(year, month, day)` of a day count |
| `last_sunday` | the day of the last Sunday of `month` in `year` |
| `berlin_summer` | whether Central European Summer Time holds at the UTC second `utc` |
| `offset` | the offset of `zone` from UTC at `utc`, in seconds |
| `local_midnight_utc` | the UTC second of 00:00 on the first of `month` in `zone` |
| `month_start_seconds`, `next_month_start_seconds` | `month_start` and `next_month_start` in seconds |
| `seconds` | whole seconds of an instant |

Constants: `DAY_SECONDS` (86400), `HOUR_SECONDS` (3600), `NS_PER_SECOND` (1000000000).

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

test "civil dates round-trip through days since the epoch" {
  assert days_from_civil(2026, 10, 31) == 20757
  assert civil_from_days(20757) == (2026, 10, 31)
  assert offset("Europe/Berlin", 1774746000) == Some(7200)
}
```
