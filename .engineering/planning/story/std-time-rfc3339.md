---
format: aep.planning-md/3
id: story:std-time-rfc3339
kind: story
status: draft
title: std.time writes and reads RFC 3339
revision: 1
---
# Story: std.time writes and reads RFC 3339

## Outcome

`std.time` turns a point in time into RFC 3339 text and back, so a program that prints a timestamp does not build it from `civil_from_days` and its own zero padding.

## Why

Requested on 2026-11-12 by the author of a command-line tool written in Ignite: it wrote a two-digit pad and an `rfc3339` function over `civil_from_days`. `std/time.ignite` has `days_from_civil`, `civil_from_days`, `seconds`, `offset` and the month functions, and no formatter or parser.

## Acceptance

- `rfc3339(seconds) -> Text` writes UTC as `YYYY-MM-DDTHH:MM:SSZ`; a form with a fraction takes the unit `std.time` already counts in.
- `parse_rfc3339(text) -> Result<…>` reads `Z` and a numeric offset, upper and lower case `T` and `Z` as RFC 3339 section 5.6 allows, and refuses a month 13, a day 30 in February, a second 61 and a missing offset with an error that names the position.
- A property test on generated instants: `parse_rfc3339(rfc3339(t)) == t`. The examples of RFC 3339 section 5.8 are tests.
- The requirements claimed are listed through `std.rfc` and `ignite conformance` reports them.
- `docs/slice.md` section; `task gate` green.

## Scope

- `std/time.ignite`, `std/rfc/` (RFC 3339), `docs/slice.md`. Inferred.

## Not in this story

- Format strings (`strftime`), local time zones beyond the offsets `std.time` has, durations as text.
