# UX round R-01

Written by `task ux-round ROUND=R-01` (`site::round::write`) from the gating suite (`task ux`'s Playwright run, `target/ux/metrics.json`) and the trial (`ux/round/sessions.spec.ts`: 15 sessions, the cards below at every viewport). Do not edit; run the command again.

- Last round: none.
- Questionnaire: not run (no person, no keys).
- L5 probe: not run (no person, no keys).
- Recording: Playwright traces (rrweb is not in the lockfile).
- Gating viewports: 1440x900, 390x844. Measured only, reported and not gated: 1280x800, 1024x768, 768x1024.
- Frames: each taken 0.75 s after its moment (the start and every click, wheel turn, opened menu, typed text or Enter), at `prefers-reduced-motion: reduce`.

## Bars

SKILL.md section 4. A gating cell is `met` or `missed`; a measured-only cell is `reported`; `not measured` where nothing ran.

| bar | metric | limit | 1440x900 | 390x844 | 1280x800 (measured) | 1024x768 (measured) | 768x1024 (measured) |
|---|---|---|---|---|---|---|---|
| L2: no horizontal page scroll, nothing past its box | `layout.overflow` | 0 | 0 met | 0 met | 0 reported | 0 reported | 0 reported |
| L2: no text or control cut off | `layout.truncated` | 0 | 0 met | 0 met | 0 reported | 0 reported | 0 reported |
| L2: always-visible controls in view, opaque, uncovered | `always_visible.misses` | 0 | 0 met | 0 met | 0 reported | 0 reported | 0 reported |
| L2: axe-core serious or critical violations | `axe.serious_critical` | 0 | 0 met | 0 met | not measured | not measured | not measured |
| L2: motion under reduced motion | `reduced_motion.violations` | 0 | 0 met | 0 met | not measured | not measured | not measured |
| requests off the local server (L2 and the trial) | `external_requests` | 0 | 0 met | 0 met | 0 reported | 0 reported | 0 reported |
| L3 suite: the screen-only user reaches each task's truth, pointer and keyboard | `suite.tasks_reached` | all | 7/7 met | 7/7 met | not measured | not measured | not measured |
| trial: sessions that reach every task's truth | `trial.sessions_completed` | all | 3/3 met | 3/3 met | 3/3 reported | 3/3 reported | 3/3 reported |
| trial: keyboard-alone sessions that reach every task's truth | `trial.keyboard_completed` | all | 1/1 met | 1/1 met | 1/1 reported | 1/1 reported | 1/1 reported |
| trial: actions one pointer step takes, max (every fix at most 2 actions) | `trial.step_actions.max` | ≤ 2 | 13 missed | 23 missed | 14 reported | 15 reported | 13 reported |

## Ratchet

The gating suite's values against `ux/baseline.json` (`site::ux::ratchet` gates them) and the last round (none). Lower is better.

| scenario | metric | R-01 | baseline | tolerance | last round | verdict |
|---|---|---|---|---|---|---|
| design/1440x900 | `glance_cost.median` | 1 | 1 | 0 | none | same |
| design/1440x900 | `glance_cost.p95` | 1 | 1 | 0 | none | same |
| design/1440x900 | `keyboard.actions` | 5 | 5 | 0 | none | same |
| design/1440x900 | `screen_only.actions` | 1 | 1 | 0 | none | same |
| design/1440x900 | `screen_only.looks` | 1 | 1 | 0 | none | same |
| design/1440x900 | `screen_only.steers` | 0 | 0 | 0 | none | same |
| design/1440x900 | `screen_only.time_to_act_steps.max` | 1 | 1 | 5% | none | same |
| design/390x844 | `glance_cost.median` | 3 | 3 | 0 | none | same |
| design/390x844 | `glance_cost.p95` | 3 | 3 | 0 | none | same |
| design/390x844 | `keyboard.actions` | 7 | 7 | 0 | none | same |
| design/390x844 | `screen_only.actions` | 2 | 2 | 0 | none | same |
| design/390x844 | `screen_only.looks` | 2 | 2 | 0 | none | same |
| design/390x844 | `screen_only.steers` | 1 | 1 | 0 | none | same |
| design/390x844 | `screen_only.time_to_act_steps.max` | 2 | 2 | 5% | none | same |
| diagnostic-search/1440x900 | `glance_cost.median` | 1 | 1 | 0 | none | same |
| diagnostic-search/1440x900 | `glance_cost.p95` | 1 | 1 | 0 | none | same |
| diagnostic-search/1440x900 | `keyboard.actions` | 66 | 66 | 0 | none | same |
| diagnostic-search/1440x900 | `screen_only.actions` | 7 | 7 | 0 | none | same |
| diagnostic-search/1440x900 | `screen_only.looks` | 3 | 3 | 0 | none | same |
| diagnostic-search/1440x900 | `screen_only.steers` | 0 | 0 | 0 | none | same |
| diagnostic-search/1440x900 | `screen_only.time_to_act_steps.max` | 1 | 1 | 5% | none | same |
| diagnostic-search/390x844 | `glance_cost.median` | 1 | 1 | 0 | none | same |
| diagnostic-search/390x844 | `glance_cost.p95` | 3 | 3 | 0 | none | same |
| diagnostic-search/390x844 | `keyboard.actions` | 16 | 16 | 0 | none | same |
| diagnostic-search/390x844 | `screen_only.actions` | 8 | 8 | 0 | none | same |
| diagnostic-search/390x844 | `screen_only.looks` | 4 | 4 | 0 | none | same |
| diagnostic-search/390x844 | `screen_only.steers` | 1 | 1 | 0 | none | same |
| diagnostic-search/390x844 | `screen_only.time_to_act_steps.max` | 2 | 2 | 5% | none | same |
| faq-platforms/1440x900 | `glance_cost.median` | 3 | 3 | 0 | none | same |
| faq-platforms/1440x900 | `glance_cost.p95` | 3 | 3 | 0 | none | same |
| faq-platforms/1440x900 | `keyboard.actions` | 40 | 40 | 0 | none | same |
| faq-platforms/1440x900 | `screen_only.actions` | 13 | 13 | 0 | none | same |
| faq-platforms/1440x900 | `screen_only.looks` | 13 | 13 | 0 | none | same |
| faq-platforms/1440x900 | `screen_only.steers` | 12 | 12 | 0 | none | same |
| faq-platforms/1440x900 | `screen_only.time_to_act_steps.max` | 13 | 13 | 5% | none | same |
| faq-platforms/390x844 | `glance_cost.median` | 3 | 3 | 0 | none | same |
| faq-platforms/390x844 | `glance_cost.p95` | 3 | 3 | 0 | none | same |
| faq-platforms/390x844 | `keyboard.actions` | 42 | 42 | 0 | none | same |
| faq-platforms/390x844 | `screen_only.actions` | 23 | 23 | 0 | none | same |
| faq-platforms/390x844 | `screen_only.looks` | 23 | 23 | 0 | none | same |
| faq-platforms/390x844 | `screen_only.steers` | 22 | 22 | 0 | none | same |
| faq-platforms/390x844 | `screen_only.time_to_act_steps.max` | 23 | 23 | 5% | none | same |
| http-example/1440x900 | `glance_cost.median` | 1 | 1 | 0 | none | same |
| http-example/1440x900 | `glance_cost.p95` | 3 | 3 | 0 | none | same |
| http-example/1440x900 | `keyboard.actions` | 76 | 76 | 0 | none | same |
| http-example/1440x900 | `screen_only.actions` | 4 | 4 | 0 | none | same |
| http-example/1440x900 | `screen_only.looks` | 4 | 4 | 0 | none | same |
| http-example/1440x900 | `screen_only.steers` | 1 | 1 | 0 | none | same |
| http-example/1440x900 | `screen_only.time_to_act_steps.max` | 2 | 2 | 5% | none | same |
| http-example/390x844 | `glance_cost.median` | 3 | 3 | 0 | none | same |
| http-example/390x844 | `glance_cost.p95` | 3 | 3 | 0 | none | same |
| http-example/390x844 | `keyboard.actions` | 53 | 53 | 0 | none | same |
| http-example/390x844 | `screen_only.actions` | 6 | 6 | 0 | none | same |
| http-example/390x844 | `screen_only.looks` | 6 | 6 | 0 | none | same |
| http-example/390x844 | `screen_only.steers` | 3 | 3 | 0 | none | same |
| http-example/390x844 | `screen_only.time_to_act_steps.max` | 2 | 2 | 5% | none | same |
| install/1440x900 | `glance_cost.median` | 1 | 1 | 0 | none | same |
| install/1440x900 | `glance_cost.p95` | 1 | 1 | 0 | none | same |
| install/1440x900 | `keyboard.actions` | 4 | 4 | 0 | none | same |
| install/1440x900 | `screen_only.actions` | 1 | 1 | 0 | none | same |
| install/1440x900 | `screen_only.looks` | 1 | 1 | 0 | none | same |
| install/1440x900 | `screen_only.steers` | 0 | 0 | 0 | none | same |
| install/1440x900 | `screen_only.time_to_act_steps.max` | 1 | 1 | 5% | none | same |
| install/390x844 | `glance_cost.median` | 3 | 3 | 0 | none | same |
| install/390x844 | `glance_cost.p95` | 3 | 3 | 0 | none | same |
| install/390x844 | `keyboard.actions` | 6 | 6 | 0 | none | same |
| install/390x844 | `screen_only.actions` | 2 | 2 | 0 | none | same |
| install/390x844 | `screen_only.looks` | 2 | 2 | 0 | none | same |
| install/390x844 | `screen_only.steers` | 1 | 1 | 0 | none | same |
| install/390x844 | `screen_only.time_to_act_steps.max` | 2 | 2 | 5% | none | same |
| pages/1440x900 | `always_visible.misses` | 0 | 0 | 0 | none | same |
| pages/1440x900 | `axe.serious_critical` | 0 | 0 | 0 | none | same |
| pages/1440x900 | `external_requests` | 0 | 0 | 0 | none | same |
| pages/1440x900 | `layout.overflow` | 0 | 0 | 0 | none | same |
| pages/1440x900 | `layout.truncated` | 0 | 0 | 0 | none | same |
| pages/1440x900 | `reduced_motion.violations` | 0 | 0 | 0 | none | same |
| pages/390x844 | `always_visible.misses` | 0 | 0 | 0 | none | same |
| pages/390x844 | `axe.serious_critical` | 0 | 0 | 0 | none | same |
| pages/390x844 | `external_requests` | 0 | 0 | 0 | none | same |
| pages/390x844 | `layout.overflow` | 0 | 0 | 0 | none | same |
| pages/390x844 | `layout.truncated` | 0 | 0 | 0 | none | same |
| pages/390x844 | `reduced_motion.violations` | 0 | 0 | 0 | none | same |
| roadmap/1440x900 | `glance_cost.median` | 1 | 1 | 0 | none | same |
| roadmap/1440x900 | `glance_cost.p95` | 1 | 1 | 0 | none | same |
| roadmap/1440x900 | `keyboard.actions` | 7 | 7 | 0 | none | same |
| roadmap/1440x900 | `screen_only.actions` | 1 | 1 | 0 | none | same |
| roadmap/1440x900 | `screen_only.looks` | 1 | 1 | 0 | none | same |
| roadmap/1440x900 | `screen_only.steers` | 0 | 0 | 0 | none | same |
| roadmap/1440x900 | `screen_only.time_to_act_steps.max` | 1 | 1 | 5% | none | same |
| roadmap/390x844 | `glance_cost.median` | 3 | 3 | 0 | none | same |
| roadmap/390x844 | `glance_cost.p95` | 3 | 3 | 0 | none | same |
| roadmap/390x844 | `keyboard.actions` | 9 | 9 | 0 | none | same |
| roadmap/390x844 | `screen_only.actions` | 2 | 2 | 0 | none | same |
| roadmap/390x844 | `screen_only.looks` | 2 | 2 | 0 | none | same |
| roadmap/390x844 | `screen_only.steers` | 1 | 1 | 0 | none | same |
| roadmap/390x844 | `screen_only.time_to_act_steps.max` | 2 | 2 | 5% | none | same |
| std-fs/1440x900 | `glance_cost.median` | 1 | 1 | 0 | none | same |
| std-fs/1440x900 | `glance_cost.p95` | 3 | 3 | 0 | none | same |
| std-fs/1440x900 | `keyboard.actions` | 72 | 72 | 0 | none | same |
| std-fs/1440x900 | `screen_only.actions` | 4 | 4 | 0 | none | same |
| std-fs/1440x900 | `screen_only.looks` | 4 | 4 | 0 | none | same |
| std-fs/1440x900 | `screen_only.steers` | 1 | 1 | 0 | none | same |
| std-fs/1440x900 | `screen_only.time_to_act_steps.max` | 2 | 2 | 5% | none | same |
| std-fs/390x844 | `glance_cost.median` | 3 | 3 | 0 | none | same |
| std-fs/390x844 | `glance_cost.p95` | 3 | 3 | 0 | none | same |
| std-fs/390x844 | `keyboard.actions` | 43 | 43 | 0 | none | same |
| std-fs/390x844 | `screen_only.actions` | 6 | 6 | 0 | none | same |
| std-fs/390x844 | `screen_only.looks` | 6 | 6 | 0 | none | same |
| std-fs/390x844 | `screen_only.steers` | 3 | 3 | 0 | none | same |
| std-fs/390x844 | `screen_only.time_to_act_steps.max` | 2 | 2 | 5% | none | same |

## Measured-only viewports

The gating suite's L2 at the measured-only viewports, reported and not gated.

| scenario | metric | R-01 |
|---|---|---|
| pages/1024x768 | `always_visible.misses` | 0 |
| pages/1024x768 | `external_requests` | 0 |
| pages/1024x768 | `layout.overflow` | 0 |
| pages/1024x768 | `layout.truncated` | 0 |
| pages/1280x800 | `always_visible.misses` | 0 |
| pages/1280x800 | `external_requests` | 0 |
| pages/1280x800 | `layout.overflow` | 0 |
| pages/1280x800 | `layout.truncated` | 0 |
| pages/768x1024 | `always_visible.misses` | 0 |
| pages/768x1024 | `external_requests` | 0 |
| pages/768x1024 | `layout.overflow` | 0 |
| pages/768x1024 | `layout.truncated` | 0 |

## Trial

Three cards (SKILL.md section 3.1): two anchors, kept fixed for later rounds, and the rotating card of R-01, under `ux/scenarios/cards/`. Each runs as one session per viewport, from the landing page in the light theme.

| card | kind | mode | persona | tasks, in order | twists |
|---|---|---|---|---|---|
| `ux/scenarios/cards/anchor-keyboard-reference.json` | anchor | keyboard | A developer writing an Ignite program who works by keyboard alone. | std-fs, http-example, diagnostic-search | wrong-turn-design (before http-example): Expects examples under Design first, then returns home.; switches-theme (before diagnostic-search): Switches the theme by keyboard before searching. |
| `ux/scenarios/cards/anchor-newcomer.json` | anchor | pointer | A developer who heard of Ignite, uses a mouse, reads on whatever screen is at hand. | install, design, roadmap | switches-to-dark (before design): Reads at night: switches the landing page to the dark theme, which the book keeps.; wrong-turn-reference (before roadmap): Looks for what comes next under Reference first, then returns home. |
| `ux/scenarios/cards/r01-faq-reader.json` | rotating | pointer | An evaluator deciding whether Ignite fits, in a dark room, starting from the questions. | faq-platforms, install, std-fs | dark-from-the-start (before faq-platforms): Switches to the dark theme before reading anything.; wrong-turn-roadmap (before std-fs): Looks for the files API on the roadmap first, then returns home. |

Looks count each inspection of the screen; for the keyboard they are Tab presses. Glance cost: 1 a control in the primary region, 2 elsewhere in view, 3 only after a scroll or an opened menu.

| session | viewport | completed | tasks | actions | last round | looks | steers | time to act, max | glance median | glance p95 | step actions, max |
|---|---|---|---|---|---|---|---|---|---|---|---|
| `anchor-keyboard-reference-1440x900` | 1440x900 | yes | 3/3 | 435 | none | 417 | 0 | 57 | 0 | 0 | 58 |
| `anchor-keyboard-reference-390x844` | 390x844 | yes | 3/3 | 180 | none | 158 | 4 | 45 | 0 | 0 | 47 |
| `anchor-keyboard-reference-1280x800` | 1280x800 (measured) | yes | 3/3 | 435 | none | 417 | 0 | 57 | 0 | 0 | 58 |
| `anchor-keyboard-reference-1024x768` | 1024x768 (measured) | yes | 3/3 | 169 | none | 151 | 0 | 39 | 0 | 0 | 40 |
| `anchor-keyboard-reference-768x1024` | 768x1024 (measured) | yes | 3/3 | 167 | none | 149 | 0 | 37 | 0 | 0 | 38 |
| `anchor-newcomer-1440x900` | 1440x900 | yes | 3/3 | 8 | none | 8 | 0 | 1 | 1 | 1 | 1 |
| `anchor-newcomer-390x844` | 390x844 | yes | 3/3 | 12 | none | 12 | 4 | 2 | 1 | 3 | 2 |
| `anchor-newcomer-1280x800` | 1280x800 (measured) | yes | 3/3 | 8 | none | 8 | 0 | 1 | 1 | 1 | 1 |
| `anchor-newcomer-1024x768` | 1024x768 (measured) | yes | 3/3 | 8 | none | 8 | 0 | 1 | 1 | 1 | 1 |
| `anchor-newcomer-768x1024` | 768x1024 (measured) | yes | 3/3 | 8 | none | 8 | 0 | 1 | 1 | 1 | 1 |
| `r01-faq-reader-1440x900` | 1440x900 | yes | 3/3 | 23 | none | 23 | 13 | 13 | 1 | 3 | 13 |
| `r01-faq-reader-390x844` | 390x844 | yes | 3/3 | 37 | none | 37 | 27 | 23 | 3 | 3 | 23 |
| `r01-faq-reader-1280x800` | 1280x800 (measured) | yes | 3/3 | 24 | none | 24 | 14 | 14 | 1 | 3 | 14 |
| `r01-faq-reader-1024x768` | 1024x768 (measured) | yes | 3/3 | 25 | none | 25 | 15 | 15 | 1 | 3 | 15 |
| `r01-faq-reader-768x1024` | 768x1024 (measured) | yes | 3/3 | 23 | none | 23 | 13 | 13 | 1 | 3 | 13 |

## Worst moments

Sessions that stopped, then the three costliest pointer steps and the three costliest keyboard steps (most looks, then glance, then actions; one per control and page). `start` is the screen the user faced when it began to look for the control, `found` the screen on which it found it.

1. `r01-faq-reader-390x844` (pointer at 390x844), task faq-platforms: `click link /^FAQ$/` on `index.html`, 23 looks, glance 3, 23 actions.
   ![start, frame 2](shots/worst-1-start.jpg)
   ![found, frame 24](shots/worst-1-found.jpg)
2. `anchor-newcomer-390x844` (pointer at 390x844), task install: `click link /^Guide$/` on `index.html`, 2 looks, glance 3, 2 actions.
   ![start, frame 1](shots/worst-2-start.jpg)
   ![found, frame 2](shots/worst-2-found.jpg)
3. `anchor-newcomer-390x844` (pointer at 390x844), task design: `click link /^Design$/` on `index.html`, 2 looks, glance 3, 2 actions.
   ![start, frame 5](shots/worst-3-start.jpg)
   ![found, frame 6](shots/worst-3-found.jpg)
4. `anchor-keyboard-reference-1440x900` (keyboard at 1440x900), twist wrong-turn-design: `enter on link /^Ignite home$/ after 57 tabs` on `book/design/index.html`, 57 looks (Tab presses), glance 0, 58 actions.
   ![start, frame 6](shots/worst-4-start.jpg)
5. `anchor-keyboard-reference-1440x900` (keyboard at 1440x900), between home: `enter on link /^Ignite home$/ after 57 tabs` on `book/reference/std/http.html`, 57 looks (Tab presses), glance 0, 58 actions.
   ![start, frame 10](shots/worst-5-start.jpg)
6. `anchor-keyboard-reference-1440x900` (keyboard at 1440x900), between home: `enter on link /^Ignite home$/ after 55 tabs` on `book/reference/std/fs.html`, 55 looks (Tab presses), glance 0, 56 actions.
   ![start, frame 4](shots/worst-6-start.jpg)

## Contact sheets

The landing page and each task's truth page, reached by the pointer user, in the light and the dark theme, each 0.75 s after it was reached.

### 1440x900

![key pages at 1440x900](shots/contact-1440x900.jpg)

### 390x844

![key pages at 390x844](shots/contact-390x844.jpg)

### 1280x800 (measured only)

![key pages at 1280x800](shots/contact-1280x800.jpg)

### 1024x768 (measured only)

![key pages at 1024x768](shots/contact-1024x768.jpg)

### 768x1024 (measured only)

![key pages at 768x1024](shots/contact-768x1024.jpg)

## Questionnaire

not run (no person, no keys).

## L5 probe misses

not run (no person, no keys).

## Missing inputs

- questionnaire: not run (no person, no keys)
- L5 probe: not run (no person, no keys)
- L4 keyed replay: not run (no keys)
- person: none; the screen-only user of `ux/lib/bot.ts` drove every session (SKILL.md section 3.1, a driver bot)
- free session: not run (optional)
- rrweb: not in `ux/package-lock.json`; every session is recorded as a Playwright trace instead, `target/ux/round/traces/<session>.zip` (`npx playwright show-trace`), not committed for its size; `task ux-round` records it again
- review agents: `review-heuristics.md` and `review-report.md` are written into this directory after this command
- last round: none, R-01 is the first
