---
format: aep.planning-md/3
id: approval-record:wave-apply-test-deadlines-2026-11-07
kind: approval-record
status: draft
title: 'Option B: apply-test-deadlines alone as wave 1'
relations:
- decides: story:apply-test-deadlines
revision: 1
---
# Approval: story:apply-test-deadlines as wave 1

## Words

- The coordinator offered three options on 2026-11-07: A the 6 draft stories as the next wave, B `story:apply-test-deadlines` alone first so later gates pass on the first run, then A, C the larger drafts; recommendation B, "If you don't answer, I start B."
- Timo pasted that table back and wrote: "then dispatch first /aep:wave".

## Decision

- Option B: `story:apply-test-deadlines` moves draft -> proposed -> active and runs alone as wave 1, dispatched without a further stage-1 stop.
- The six stories of option A stay draft; they are proposed as the next wave after this one closes.
- Commits authorised: the unit's commits on `fix/apply-test-deadlines`, its PR merged into `main` after a green `task gate`, and the store commits on `main` (this record, the moves, the scope, the evidence). No tag, no release.
