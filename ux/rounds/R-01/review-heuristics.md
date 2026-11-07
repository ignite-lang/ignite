# R-01 heuristic review

Verdict: 7 pass, 5 partial, 4 fail

Inputs: `report.md`, `shots/`, the 15 Playwright traces in `target/ux/round/traces/` (screencast
frames and action timings read from the unzipped traces), the frames in
`target/ux/round/frames/`, the session logs in `target/ux/round/sessions/`, the three cards. No
operator notes, no questionnaire. Extra evidence: `shots/review-*.png`, taken with Playwright
on the built `target/site/`. Recording seconds count from the trace's context start.

| # | heuristic | verdict | evidence |
|---|---|---|---|
| 1 | One next thing | pass | `shots/worst-1-start.jpg`: one filled primary, "Get started", beside two outlined secondaries; `shots/contact-1440x900.jpg` landing tile, same in both themes |
| 2 | State in words | pass | `shots/contact-390x844.jpg` roadmap tile: "released", "unreleased", "design" written in each badge; diagnostics tile names each code ("E046 Match not exhaustive") |
| 3 | Nothing required hidden | fail | FAQ and Diagnostics linked only from the landing footer: `r01-faq-reader-390x844.zip` @ 2.4 to 25.3 s, 22 wheel turns before the click (`shots/worst-1-found.jpg`); `r01-faq-reader-1440x900.zip` @ 2.3 to 14.4 s, 12; main navigation in `shots/review-landing-header-1440x900.png` and `shots/review-menu-390x844.png` holds no FAQ. UXF-008 |
| 4 | Stable layout | pass | frames 2 to 24 of `frames/r01-faq-reader-390x844` and 1 to 15 of `frames/anchor-keyboard-reference-1440x900`: nothing moves between moments; the search results push the page down only after the reader types (`anchor-keyboard-reference-1440x900.zip` @ 48 to 50 s) |
| 5 | Provenance of every value | pass | every code block labelled "ignite · excerpt" or "ignite · fails to check, on purpose" (`shots/review-book-first-tab-1440x900.png`, `shots/contact-390x844.jpg` diagnostics tile); the landing window names its source file under the code |
| 6 | Errors cheap (at most 2 actions) | fail | pointer wrong turns undo in 1 click (`sessions/r01-faq-reader-1440x900.json`, Roadmap then Ignite home), but the keyboard wrong turn into Design costs 57 Tabs to undo (`anchor-keyboard-reference-1440x900.zip` @ 16.6 to 21.1 s, `shots/worst-4-start.jpg`); bar `trial.step_actions.max` 13 and 23 against at most 2 (report, Bars). UXF-008, UXF-009 |
| 7 | Words the user can say | partial | landing labels are words (Guide, Design, Reference, Roadmap); the book's menu bar is icons only (bars, paintbrush, magnifier, print, GitHub, pencil), named only to assistive technology ("Toggle Searchbar", "Change theme"): `shots/review-book-first-tab-1440x900.png`. Not filed: the trial found each in 1 look |
| 8 | The reader's time respected | fail | keyboard session 435 actions for three tasks at 1440x900 and 1280x800, 167 to 180 at the narrower viewports, 8 for the pointer card (report, Trial); `keyboard.actions` 66 to 76 per task at 1440x900. UXF-009 |
| 9 | Keyboard and pointer parity | fail | focus ring visible on every stop (`anchor-keyboard-reference-1440x900.zip` @ 11 to 15 s, orange outline on sidebar entries), but book pages at 1440 px put every sidebar entry before the menu bar and have no skip link: first Tab lands on "Welcome" (`shots/review-book-first-tab-1440x900.png`), "Ignite home" after 55 to 57 Tabs (`shots/worst-5-start.jpg`, `worst-6-start.jpg`). UXF-009 |
| 10 | Recognition over recall | pass | numbered table of contents always in view on wide book pages and one Tab or click away on narrow ones; search finds "E046" by its code (`anchor-keyboard-reference-1440x900.zip` @ 48 to 50 s, "2 search results for 'E046'") |
| 11 | Calm by default | pass | `reduced_motion.violations` 0 at both gating viewports (report, Bars); no banner, toast or animation in any frame of `frames/` |
| 12 | One state vocabulary | partial | the theme is a two-state sun/moon toggle "Switch between dark and light theme" on the landing page and a paintbrush menu "Change theme" with Auto, Light, Dark in the book (`site/book/theme/index.hbs`, `shots/review-book-first-tab-1440x900.png`); the same book is "Guide" in the header, "Book" in the footer, "The Ignite Book" in its own bar (`shots/worst-1-found.jpg`). Not filed: no session lost time to it |
| 13 | A wrong cue recovered in one action | partial | pointer: the Design, Reference and Roadmap wrong turns are undone by one click on "Ignite home" (`sessions/anchor-newcomer-1440x900.json` steps 5, 7); keyboard at 1440 px: 57 Tabs (`anchor-keyboard-reference-1440x900.zip` @ 16.6 to 21.1 s). UXF-009 |
| 14 | Unmistakable end state | pass | every truth page opens on an h1 that names it (std.fs, std.protocol.http, Install from source, The idea, Roadmap, FAQ: `shots/contact-1440x900.jpg`, `contact-390x844.jpg`); the E046 entry is headed "E046 Match not exhaustive" |
| 15 | Readable in every theme; key data at 14 px or more | partial | both themes readable on every tile of the five contact sheets; key data under 14 px: book inline code 11.9 px at 390x844 and 13.3 px at 1440x900, roadmap badges 10.0 to 11.9 px, landing code 12.5 px (computed sizes on `target/site/`; `shots/contact-390x844.jpg` std-fs and roadmap tiles). UXF-010 |
| 16 | Focus follows the work | partial | the search toggle moves focus into the search field and the results take the next Tab (`anchor-keyboard-reference-1440x900.zip` @ 47 to 50 s); on arrival at a wide book page focus starts at the top of the sidebar, not at the content or the menu bar (`shots/review-book-first-tab-1440x900.png`). UXF-009 |

## Findings filed

| id | title | severity | class | sessions | cost |
|---|---|---|---|---|---|
| UXF-008 | The FAQ is linked from the landing page's footer only | S2 | affordance | `r01-faq-reader` at all 5 viewports | S |
| UXF-009 | On wide book pages a keyboard user tabs through the whole table of contents before the menu bar | S2 | affordance | `anchor-keyboard-reference-1440x900`, `-1280x800` | S |
| UXF-010 | Code names and status badges are smaller than 14 px | S3 | layout | every session reaching a reference page or the roadmap; all at 390x844 | M |

## Notes on the evidence

- The dark-theme twists did not run in the dark theme past their first page. The trial's
  `chooseTheme` (`ux/lib/browse.ts:28`, called from `ux/round/sessions.spec.ts:107`) is a
  context init script that writes `mdbook-theme = light` before every page load, so the
  reader's choice is overwritten on each navigation: `frames/r01-faq-reader-1440x900` frame 2
  is dark, frames 15 (FAQ) and 16 (landing) are light; `frames/anchor-newcomer-1440x900`
  frames 5 to 8 likewise. The site itself keeps the choice:
  `shots/review-theme-kept-faq-1440x900.png` (landing toggled to dark, FAQ opens in `coal`,
  home stays dark). No finding filed against the site; heuristic 15's dark verdict rests on the
  contact sheets, which set the theme before loading.
- The 38 and 45 Tabs to "Guide" on the landing page after the theme switch
  (`sessions/anchor-keyboard-reference-1440x900.json`, `-390x844.json`) come from the bot
  tabbing forward from the theme toggle and wrapping round the page; a few Shift+Tab presses reach
  it. Not filed.
