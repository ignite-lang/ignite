---
format: aep.planning-md/3
id: task:r01-book-skip-link
kind: task
status: implemented
title: 'R-01: keyboard reaches the book menu bar first'
relations:
- decomposes: story:docs-site-ui-loop
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-07T16:41:07Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-07T16:41:07Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-11-07T17:29:45Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
# Task: r01-book-skip-link

## Outcome

On book pages the first Tab reaches the menu bar (a skip link, or the menu bar before the sidebar in tab order).

## Context

- Round R-01 change set row 2, `ux/rounds/R-01/triage.md`; finding `ux/findings/UXF-009.md`.

## Acceptance

- `ux/tests/regressions.spec.ts :: UXF-009` is red before the change and green after; `task ux` exit 0.
- In R-02, `anchor-keyboard-reference` reaches the book's home link or search in under 5 Tabs at 1440x900 (R-01: 55 to 57).
