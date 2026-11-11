---
format: aep.planning-md/3
id: approval-record:wave-11-size-rule-2026-11-11
kind: approval-record
status: draft
title: Wave 11 takes the size rule as its eleventh story
relations:
- decides: story:size-rows-grow-with-a-decision
revision: 1
---
# Approval: wave 11 takes the size rule as its eleventh story

## Words

- Timo, 2026-11-11: "increase in binary size isn't a big deal. Yeah, I mean, if we're adding more
  code and more logic, then obviously the size will increase, right? But what would be bad is
  execution time, compile time, relative to the file size."
- Timo, 2026-11-11, the session's goal: "new release cut, no performance/quality drop".

## What it covers

`story:size-rows-grow-with-a-decision`: the release check and the dedicated-host check stop
refusing a release for a binary-size row (3.2, 3.9) above the last release when the bar carries a
`decision` that names the added code. Unit size ends with six Rust cells above v0.15.0 after it
reverted every change that bought bytes with instructions; without this story the release check
refuses v0.16.0 for them.

## The coordinator's reading

The owner's words put time before bytes and accept growth that comes from added code. The
coordinator keeps growth deliberate (a grown cell without a `decision` is still refused), takes
the story into the wave as the eleventh unit and reported the decision to the owner (D33 and D36
of the session's reports). The owner has not been asked separately; if he reads the rule
otherwise, the story's merge is reverted alone.
