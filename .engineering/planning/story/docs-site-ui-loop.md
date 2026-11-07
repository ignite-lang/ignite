---
format: aep.planning-md/3
id: story:docs-site-ui-loop
kind: story
status: active
title: The public website runs the measured UI improvement loop
scope:
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: crates/ignite/tests/site.rs
- confidence: cited
  path: site
- confidence: cited
  path: ux
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-07T13:44:00Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-07T13:44:00Z", actor: "human:timo", revision: 3}
---
# Story: docs-site-ui-loop

## Outcome

The public website (`site/`: landing page `site/index.html` and the mdBook under `site/book/`, built by `task site` into `target/site/`) is under the measured UI improvement loop of the `ui-iteration-loop` skill: every merge runs hermetic layout, accessibility and screen-only-user gates against a ratcheted baseline, and each round's findings become regression assets.

## Context

- Timo, 2026-11-07: "/ui-iteration-loop for pub docs".
- Skill: `~/.claude/skills/ui-iteration-loop/SKILL.md` and its `references/` (`heuristics.md`, `review-agents.md`, `not-available.md`).
- Existing site checks: `crates/ignite/tests/site.rs` (`site::`: every `ignite` block checks or is marked, no page names `target/debug/ignite`).
- Tools on the machine: node 22, Playwright Chromium in `~/.cache/ms-playwright`, mdbook 0.5.4, miniserve, ffmpeg.
- Repository rule: tooling and checks are Rust; the skill allows Playwright specs in TypeScript.

## Adapter

| slot | value |
|---|---|
| scenarios | `ux/scenarios/<name>/`: the page list from the built `target/site` (landing page and every book page) and tasks such as "find the install command", "find the reference page of `std.fs`", "find the HTTP example" |
| gating viewports | 1440x900, 390x844 |
| measured-only viewports | 1280x800, 1024x768, 768x1024 |
| themes | every theme the site ships: light and dark (the book menu offers Auto, Light and Dark, `coal`; the landing page maps `coal` to dark) |
| always-visible | landing page navigation; book: menu bar, search toggle, theme toggle; sidebar at desktop sizes |
| guidance cue | none: no L0, no gate 6 |
| baseline | `ux/baseline.json` |
| rounds, findings | `ux/rounds/R-nn/`, `ux/findings/UXF-nnn.md` |

## Acceptance

- `task ux` builds the site and runs, hermetic and without keys, at both gating viewports and every theme: no horizontal page scroll or element past its box; the always-visible elements 100 % in view and uncovered; axe-core 0 serious or critical violations (`wcag2a, wcag2aa, wcag21a, wcag21aa, wcag22aa`); every task completed by keyboard alone; the screen-only user completes every task from the landing page using only visible controls; the ratchet against `ux/baseline.json` exits 1 on a worse, missing or untaken-improved value; `task gate` runs `task ux`.
- A test fails on a closed S1/S2 finding record without a regression asset.
- Round R-01 written under `ux/rounds/R-01/` (report, `round.json`, shots, `review-heuristics.md`, `review-report.md`, `triage.md`) with its change set as tasks under this story.
