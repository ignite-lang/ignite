# Review of the R-01 report

Reviewer: report reviewer (`references/review-agents.md` section 2), 2026-11-07. Inputs: `report.md`,
`round.json`, `shots/`, the 15 Playwright traces and session records under `target/ux/round/`,
`ux/README.md`, `trend.md`, and the source of `ux/round/sessions.spec.ts`, `ux/round.config.ts`,
`ux/lib/*.ts`, `crates/ignite/tests/site/round.rs` and the Taskfile target `ux-round`. No last
round, no operator notes, no questionnaire. Nothing paid or live was run; `task ux-round` was not
re-run, because the traces were enough to recompute the numbers.

**Verdict: not fit to send as is.** The counts are right. But the trial undoes every theme twist on
the next page load (T1), so the rotating card's "dark room" persona and the newcomer anchor's
"which the book keeps" were never tested. Default action: fix T1 in `ux/round/sessions.spec.ts`,
re-run `task ux-round ROUND=R-01`, and send with the caveats in section 4. The action counts should
stay the same, because a theme does not move any control. The frames, the `theme` fields and the
worst-moment shots after each twist will change.

## 1. Are the numbers right and meaningful?

Recomputed from the traces (`trace.trace` `before` events: `mouseClick`, `mouseWheel`,
`keyboardPress` split by key) and compared with the report.

| number | from the traces | report | difference |
|---|---|---|---|
| actions, every one of the 15 sessions | 8, 8, 8, 12, 8 / 169, 435, 435, 180, 167 / 25, 24, 23, 37, 23 | same | none |
| keyboard looks = Tab presses | 151, 417, 417, 158, 149 | same | none |
| pointer steers = wheel turns + menu opens | r01 390x844: 23 wheels + 4 opens = 27; the other r01 sessions: 13, 14, 15 wheels | 27, 13, 14, 15 | none |
| frames = screenshots | 9 to 38 per session | same count in `frames` | none |
| trend: trial pointer actions (gating) | 8 + 12 + 23 + 37 = 80 | 80 | none |
| trend: trial keyboard actions (gating) | 435 + 180 = 615 | 615 | none |
| trend: worst pointer looks, worst keyboard tabs | 23 (FAQ, 390x844), 57 (Ignite home, 1440x900) | 23, 57 | none |
| bar `trial.step_actions.max` | 13 at 1440x900, 23 at 390x844 (both on the FAQ step) | 13, 23 missed | none |

`target/ux/metrics.json` (16:42:10) was written before the trial (16:43), so the ratchet and the
L2/L3 bars come from this run. Whether the gating suite passed cannot be read from any round output
(T5).

What the metrics mean in this round:

- **Pointer looks, time to act and step actions repeat one number.** In `lib/bot.ts`
  `pointerClick`, every look either clicks or makes one steer. So a pointer session's looks equal
  its actions (all 10 pointer sessions), and a step's time to act equals its step actions
  (13/13, 23/23). Three columns hold one measurement. Only the keyboard looks (Tab presses) are
  separate from actions.
- **The keyboard glance is 0, not a cost.** The legend says glance runs from 1 to 3. The keyboard
  rows show `glance median 0 / p95 0` because `bot.ts` writes 0 for the keyboard (T2).
- **The theme twists measure nothing after the landing page** (T1). Every frame after a twist on a
  book page has `theme: light`, in all 15 sessions. For example, `r01-faq-reader-1440x900` shows
  frames 2 to 14 dark and frames 15 to 24 light.
- **The ratchet's "same" on `faq-platforms` is not a pass.** The baseline already holds 13 (1440)
  and 23 (390) actions for that task, so the ratchet treats the worst path of the round as
  normal. The only signal is the step-actions bar, which is missed.
- **The two missed bars are one finding**: the FAQ step at 1440x900 and at 390x844.
- **1440x900 and 1280x800 give the same results**, keyboard 435 actions at both. They are on the
  same side of the book's 1080 px sidebar width, so the extra viewport adds no information this
  round.
- **The `sessions completed` column (15/15) counts the measured-only viewports.** The trend's
  header says only the action sums are limited to the gating viewports.

## 2. Findings the report misses

1. **The FAQ link is only in the footer, at every viewport.** `site/index.html:458` is the one FAQ
   link; the main navigation (lines 33 to 39) has none. Getting to it took 12 wheel turns at
   1440x900 and 768x1024, 13 at 1280x800, 14 at 1024x768 and 22 at 390x844 (trace wheel counts,
   `r01-faq-reader-*`). The report shows only the 390x844 case. The 1440x900 case (13 actions, a
   gating bar miss) has no shot, because the worst-moment de-duplication ignores the viewport (T3).
   The `found` shot `shots/worst-1-found.jpg` shows the footer row "Book FAQ Diagnostics GitHub".
2. **Keyboard on wide book pages: about 55 Tabs to reach the menu bar.** At 1440x900 and 1280x800,
   reaching `Ignite home` takes 55 to 57 Tabs and `Toggle Searchbar` 55. At 1024x768, 768x1024
   and 390x844 they take 3 to 4 Tabs. That is why the keyboard session costs 435 actions at the
   wide viewports and 167 to 180 at the others. Inferred, not checked in the DOM: above the 1080 px
   sidebar width, the sidebar table of contents comes before the menu bar in Tab order, and no
   skip link passes it. The report lists the three 57/55-Tab steps but not the viewport split or
   the cause.
3. **The dark theme does reach the book; the harness hides it.** Checked in scratch with the built
   site: a click on the landing toggle stores `coal`, and `book/design/index.html` then opens with
   `class="... coal ..."`. With the trial's init script added, the same steps give `light`. So
   "the book keeps the theme" is true of the site but was not shown by this round. No session has
   a dark book frame. Dark book pages appear only in the contact sheets, which are seeded dark by
   address.
4. **At 390x844 every landing-page route opens the menu first** (glance 3, 4 opens per session in
   both anchor sessions and the r01 session). The report's worst moments 2 and 3 show this as two
   separate items, but it is one pattern.

## 3. Tool defects

- **T1, the trial resets the theme on every page load.** `ux/round/sessions.spec.ts` calls
  `chooseTheme(context, themes[0])` (`ux/lib/browse.ts`). That uses `context.addInitScript`, which
  writes `mdbook-theme = light` before every navigation, so any theme switch is lost when the next
  page loads. Shown by: card `anchor-newcomer` twist `switches-to-dark`, session
  `anchor-newcomer-1440x900` frames 4 (`dark index.html`) and 5 (`light book/design/index.html`).
  Confirmed in scratch: with the init script the book shows `light`; without it, `coal`. Fix:
  seed the theme only when storage is empty, or once through `storageState`.
- **T2, the keyboard glance is shown as 0.** `ux/lib/bot.ts` `keyboardClick` pushes `glance: 0`,
  and `crates/ignite/tests/site/round.rs` (session table, `glance_median`/`glance_p95`) prints it
  under a legend that says 1 to 3. Input: any keyboard session, for example
  `anchor-keyboard-reference-1440x900` with `0 | 0`. It should print "n/a".
- **T3, worst moments drop the same control at other viewports.** In `round.rs` `worst`, the
  `distinct` key is `(label, page)` without the viewport. Input: `r01-faq-reader-1440x900` FAQ step
  (13 looks, gating, bar missed) is dropped in favour of 2-look steps (ranks 2 and 3). Also, the
  trend's `worst_pointer_looks` and `worst_keyboard_tabs` (`worst_of`) are taken from every
  viewport, measured ones included, while the trend's header says gating only.
- **T4, the external-requests bar reads 0 when there is no data.** `round.rs` `bars`,
  `external_requests`: `l2.unwrap_or(0) + trial` gives "0 met" when `target/ux/metrics.json` is
  missing. Input: delete `target/ux/metrics.json` and run `site::round::write`. The L2 bars then
  say "not measured", but this one says "0 met".
- **T5, the gating suite's result is not recorded.** In `Taskfile.yml` `ux-round`, the gating run
  is `ignore_error: true`. `ux/README.md` says "a failure there is reported", but `round.rs` reads
  only `metrics.json`. Failures in the contract, adversary or regression tests leave no trace. If
  the run fails before its global teardown (for example `miniserve` not on `PATH`, so
  `lib/server.ts` throws), the `metrics.json` of an earlier run is still there and is reported as
  this round's. Its timestamp is not checked against the trial.
- **T6, keyboard worst moments have no `found` shot, and the report does not say why.** `round.rs`
  `worst` adds `found` only for pointer steps, and a Tab press is not a frame in `bot.ts`. The
  report text says "`found` the screen on which it found it". Input: worst moments 4 to 6.
- **T7, `report.md` claims "Each runs ... from the landing page in the light theme"** for every
  card, while the rotating card's first twist makes the whole persona dark. Together with T1, the
  report's card table describes sessions that did not happen as written.
- **T8 (minor), the contact sheet is not readable at the wide viewports.** `sessions.spec.ts`
  `THUMB_WIDTH = 320` shrinks a 1440 px page to 22 %, and `shots/contact-1440x900.jpg` text can't
  be read. It is good enough for checking the theme and layout, not the content.

## 4. Caveats to send with the round

- No person, no questionnaire, no L5 probe, no keyed replay. Every session is the deterministic
  bot, so "looks" are a model's inspections, not a reader's.
- Until T1 is fixed, no session frame shows a book page in the dark theme. The theme twists are
  measured on the landing page only.
- For pointer sessions, looks, time to act and step actions are one number shown three times.
- The ratchet is "same" everywhere because the baseline was blessed from this site state,
  including the 13- and 23-action FAQ path.
- Whether the gating suite passed in this run is not recorded (T5).
- 1280x800 repeats 1440x900 exactly this round.
