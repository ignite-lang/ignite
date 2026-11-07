---
format: aep.planning-md/3
id: task:r01-faq-link
kind: task
status: implemented
title: 'R-01: FAQ link in the landing header'
relations:
- decomposes: story:docs-site-ui-loop
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-07T16:41:07Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-07T16:41:07Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-11-07T17:29:45Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
# Task: r01-faq-link

## Outcome

The landing page's header navigation links the FAQ, in view without scrolling at every viewport.

## Context

- Round R-01 change set row 1, `ux/rounds/R-01/triage.md`; finding `ux/findings/UXF-008.md`.

## Acceptance

- `ux/tests/regressions.spec.ts :: UXF-008` is red before the change and green after; `task ux` exit 0.
- In R-02, `r01-faq-reader` takes at most 2 actions per step at 1440x900 and 390x844 (R-01: 13 and 23).
