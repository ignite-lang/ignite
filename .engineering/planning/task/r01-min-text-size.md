---
format: aep.planning-md/3
id: task:r01-min-text-size
kind: task
status: implemented
title: 'R-01: code and badge text at least 14 px'
relations:
- decomposes: story:docs-site-ui-loop
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-07T16:41:07Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-07T16:41:07Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-11-07T17:29:45Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
# Task: r01-min-text-size

## Outcome

Inline code, roadmap badges and landing-page code have a computed size of at least 14 px at every viewport.

## Context

- Round R-01 change set row 3, `ux/rounds/R-01/triage.md`; finding `ux/findings/UXF-010.md`.

## Acceptance

- `ux/tests/regressions.spec.ts :: UXF-010` is red before the change and green after; `task ux` exit 0.
- In R-02, the smallest computed size of those elements is at least 14 px (R-01: 10.0 px).
