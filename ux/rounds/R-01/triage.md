# R-01 triage

Applied by rule (SKILL.md § 3.3) by the coordinating session, 2026-11-07.

## Caveats

- No person and no keys: the questionnaire, L4 and L5 did not run. Every session was driven by the screen-only user.
- T1 (`review-report.md` § 3): the trial reset every page to the light theme, so the dark-theme steps of `r01-faq-reader` and `anchor-newcomer` were not measured. The site keeps the choice (`shots/review-theme-kept-faq-1440x900.png`). The tool defects T1 to T8 are fixed before R-01 is written again. They are tool changes and are not counted among the 5 changes.
- R-01 was written twice: first at ac1d7c88, again after the T1 to T8 fixes (aa2fc7a1). `review-heuristics.md`, `review-report.md`, UXF-008 and UXF-009 were written against the first write. The `shots/worst-N-*.jpg` names they cite now hold the second write's moments; the first write's frames are in git at ac1d7c88. The `shots/review-*.png` files are unchanged.
- Frequency = sessions affected ÷ 15 sessions (3 cards × 5 viewports, `round.json` `sessions`).

## Did the last change sets move their metrics?

R-01 is the first round; there is no earlier change set.

## Findings

| finding | severity (weight) | sessions | frequency | confidence | cost | priority |
|---|---|---|---|---|---|---|
| UXF-008 FAQ linked from the footer only | S2 (3) | `r01-faq-reader` × 5 | 5/15 = 0.33 | 2 (step-action bar missed in `report.md`, heuristic review, report review) | S (1) | 2.00 |
| UXF-009 wide book pages tab through the table of contents first | S2 (3) | `anchor-keyboard-reference` at 1440x900 and 1280x800 | 2/15 = 0.13 | 2 (Tab counts in `report.md`, heuristic review, report review) | S (1) | 0.80 |
| UXF-010 code names and badges under 14 px | S3 (1) | not counted; at most 15 | ≤ 1 | 1 (heuristic review) | M (2) | ≤ 0.50 |

No S1 finding. No finding needs a design decision, so there is no `decision.md`.

## Change set R-01 (3 of at most 5)

| # | change | finding | regression asset | target metric |
|---|---|---|---|---|
| 1 | a FAQ link in the landing page's header navigation, reachable without scrolling at every viewport | UXF-008 | `ux/tests/regressions.spec.ts :: UXF-008` test, red first | `r01-faq-reader` step actions at most 2 at 1440x900 and 390x844 (now 13 and 23) |
| 2 | book pages: a skip link to the menu bar, or the menu bar before the sidebar in tab order | UXF-009 | `ux/tests/regressions.spec.ts :: UXF-009` test, red first | `anchor-keyboard-reference` Tabs to the book's home link or search at 1440x900 under 5 (now 55 to 57) |
| 3 | inline code, roadmap badges and landing code at least 14 px computed | UXF-010 | `ux/tests/regressions.spec.ts :: UXF-010` test, red first | smallest computed text size of those elements at least 14 px at every viewport (now 10.0 px) |

Each change becomes one task under `story:docs-site-ui-loop`. Its acceptance is the regression asset red before and green after, plus the target metric moving in R-02.
