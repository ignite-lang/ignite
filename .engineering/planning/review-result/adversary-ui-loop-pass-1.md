---
format: aep.planning-md/3
id: review-result:adversary-ui-loop-pass-1
kind: review-result
status: active
title: Adversary, docs-site-ui-loop part 1, pass 1
relations:
- reviews: story:docs-site-ui-loop
revision: 1
---
unit: story:docs-site-ui-loop part 1, worktree ui-loop at 18047869 (base 8ca047d4), plus my two uncommitted test files
verdict: NEEDS-CHANGE (blocker: the overflow gate cannot fire on any book page)
cases: executed 258→265 (Playwright 253→259, `site::ux::` 5→7), red 6
origin: introduced 6 / pre-existing 0 / undecided 0
wrote-outside-worktree: 1 directory tree (the assigned scratch directory, listed in part 6)
needs-coordinator: R-01 (`ux/rounds/R-01/`, acceptance item 3) does not exist; I took it as outside part 1 and did not attack it

## 1. Diff

```
 crates/ignite/tests/site/ux.rs | 36 ++++++++++++++++++++++++++++++++++++
 1 file changed, 36 insertions(+)
?? ux/tests/adversary.spec.ts   (new, untracked)
```

Both are test files. I edited no implementation file. Every site mutation was made on copies in scratch, never in the tree, so there was nothing to restore.

## 2. Cases added (red output from running each case alone, before any suite run)

`ux/tests/adversary.spec.ts`:

1. **Overflow inside mdBook's content column** — red: `Error: no problem reported for a 2400 px element in a 400 px reading column … Expected: > 0  Received: 0`. The fixture copies the built book's nesting: body `overflow-x:hidden`, then `overflow-x:clip`, then `#mdbook-content` `overflow:auto`.
2. **Always-visible check, control at opacity 0** — red: `Expected ["always visible"]  Received []`.
3. **Always-visible check, control covered everywhere but a 10 px strip at its centre** — red: `Received []`. My first version of this case was wrong (the body margin moved the centre under a cover box). I corrected it and reran it alone before any suite run.
4. **Screen-only user, opacity-0 link** — red: `Received promise resolved instead of rejected … {"actions": 1, "log": ["click link /^Goal$/"]}`. The bot clicked a link nobody can see.
5. **Keyboard user at 390, http-example: every Enter lands on a focused control inside the viewport** — green. I could not break this.
6. **Book at 1440: menu bar and toggles stay in view after scrolling down 2100 px** — green. I could not break this.

`crates/ignite/tests/site/ux.rs`:

7. `adversary_a_tolerance_raised_against_mains_needs_a_decision` — red: `panicked at ux.rs:596:5: 7 serious axe violations pass against main's 0 by a raised tolerance and no decision`.
8. `adversary_a_regression_asset_naming_no_test_is_refused` — red: `panicked at ux.rs:609:5: a closed S2 finding whose asset is README.md passes the findings check`.

## 3. Suite runs (after the cases existed)

- **Playwright, full suite on a scratch copy of the harness and site, with my spec:** `npx --no-install playwright test --workers=10` gave `4 failed, 255 passed (1.3m)`, exit 1. The four failures are cases 1 to 4.
- **The same run without my spec:** `253 passed`, exit 0. This is the "before" count.
- **Rust:** `cargo test -q -p ignite --test main -- site::ux::` gave `FAILED. 5 passed; 2 failed; 1 ignored`, exit 101. The two failures are cases 7 and 8.

## 4. Findings

Findings cover worktree 18047869.

1. **ux/lib/layout.ts:42-48 — the overflow gate is blind on every book page. Blocker, NEEDS-CHANGE, introduced.**
   - **Measured:**
     - Adding `<div style="width:2400px">` to `book/faq.html` passed L2 at both gating viewports.
     - A probe of that page showed the root at 1440/1440 while `#mdbook-content` (`overflow:auto`) was 2541 px wide in a 1102 px box. The whole reading column scrolls sideways.
     - Every element of a book page has a scrolling or clipping ancestor (`#mdbook-content`, `#mdbook-body-container` with `clip`, `body` with `hidden`), so the check skips them all. "No horizontal page scroll" is checked on the landing page only (2 of the 47 checked pages are not book pages).
   - **What reaches it:** any wide block added to any book page. No book page scrolls today: a probe of all of them at 1440 and 390 found none.
   - **Fix to name:** treat a scrolling ancestor that is the page's main column (or any scroller wider than its box that is not a `pre` or `table`) as page scroll.
2. **ux/lib/layout.ts:152 — the always-visible check ignores `opacity`. Warning, CONFIRMED, introduced.**
   - **Measured:** case 2 is red, and on the site copy `#mdbook-theme-toggle{opacity:0}` on `book/guide/testing.html` passed L2. `display:none` on the toggle at 1440 is caught, as asked.
   - **What reaches it:** no current page. Hover-reveal styling with opacity 0 is the common way to hide a control, and mdBook's own CSS uses `visibility:hidden; opacity:0` together.
3. **ux/lib/layout.ts:170 — "uncovered" means uncovered at one point (the centre), not 100 %. Note, CONFIRMED, introduced.**
   - **Measured:** case 3 is red.
   - **What reaches it:** nothing found on the site.
4. **ux/lib/bot.ts:78 — the screen-only user clicks controls nobody can see (opacity 0). Warning, CONFIRMED, introduced.**
   - **Measured:** case 4 is red.
   - **What reaches it:** no current task route goes through an invisible control; every L3 task passes.
5. **crates/ignite/tests/site/ux.rs:184 — the ratchet can be loosened without a decision. Warning, CONFIRMED, introduced.**
   - **Measured:** the comparison with `main` checks only `value`. Raising `tolerance` on the branch lets a worse run pass with no decision (case 7).
   - **What reaches it:** an edit of `ux/baseline.json` on any branch.
6. **crates/ignite/tests/site/ux.rs:510 — the findings check accepts any file that exists as a regression asset. Warning, CONFIRMED, introduced.**
   - **Measured:** the check tests only that the asset's path exists. `README.md :: a test that is nowhere`, an absolute path and a `..` path would all pass (case 8). The skill requires an asset that fails before the fix.
   - **What reaches it:** writing a new finding record.
7. **Taskfile.yml:144 — `task ux` is not hermetic in a new checkout. Warning, CONFIRMED, introduced.**
   - **Measured:** `ux-deps` skips installation only when `node_modules` exists. In a copy with no `node_modules`, `task --dry ux-deps` prints `npm ci --no-audit --no-fund`, which fetches from `registry.npmjs.org` (the lockfile's `resolved` URLs). Chromium revision 1254 must also already be on the machine.
   - **What reaches it:** `task gate` in every newly created managed worktree, which is how all repository work here is done. This contradicts "hermetic" in the acceptance; the README states the npm step.

## 5. Attacked and could not break

- **Covering the menu bar or search toggle on `roadmap.html`:** red, with 5 controls reported as "covered at its centre by div#adv-cover".
- **Removing the theme toggle's name on `design/index.html`:** red, axe `button-name` (critical).
- **`display:none` on the theme toggle at ≥1200 px:** red at 1440, green at 390, which is correct.
- **A page no list names (`book/adversary-extra.html`):** it got its own L2 tests. The page list comes from the build (`walk`), not a fixed list.
- **Dropping std.fs from the sidebar:**
  - Rebuilt without the SUMMARY line: L3 std-fs red at both viewports.
  - Removed from the built sidebar only: red at 390, green at 1440. At 1440 the bot finds the link in the std index table, so that pass is genuine.
- **Screen-only user:**
  - It only ever loads the start URL by address.
  - Pointer mode uses role and name plus in-view and hit tests.
  - Keyboard Enter always landed on a control in view (case 5).
  - One DOM read remains: the open state of a disclosure is read with `querySelector` (note only).
- **Hermetic network in the browser:** the built site loads no remote script, style or font; the only remote URLs are `<a href>` links to github.com.
- **Determinism:** two full runs at load 27 to 36 gave byte-identical `metrics.json`, with 0 differences from `ux/baseline.json`.
- **Themes:** the built book's menu offers only Auto, Light and Dark (`coal`), and the landing page maps `coal` to dark. Gating two themes is true of the built site. The story's list of five mdBook themes is stale, not the code.
- **Each UXF fix fails without it:** the base site's `site/` (8ca047d4) run under the unit's harness gave 95 failures:
  - UXF-001: `scrollable-region-focusable`, 796 nodes.
  - UXF-002: `aria-allowed-attr` 180 and `aria-prohibited-attr` 180.
  - UXF-003: `target-size`, 532.
  - UXF-004: `color-contrast`, 16.
  - UXF-005: 2 horizontal page scroll and 10 element past its box.
  - UXF-006: 2 truncated text.
  - UXF-007: L3 std-fs red.
  - With only UXF-002's `index.hbs` change reverted, L3 at 390 goes red in pointer mode: the bot opens the menu only through `role=button`.
- **Ratchet:** a worse value, a missing key, a new key and an improvement not written into the baseline are each refused (existing break-tests). Removing a baseline key and blessing it back at a worse value is caught by the comparison with `main`.

## 6. Paths written outside the worktree

All under `/home/timo/.cache/scratch/e18a99fc-f99c-4987-93c9-e5de2080b61a/unit-ui-loop/adversary/`:
- Scripts: `setup.sh`, `run.sh`, `mutate.js`, `drop-sidebar.js`, `twice.sh`, `compare.js`.
- Scratch copies: `clean/`, `mut/`, `mut4a/`, `mut4b/`, `base/`, `r002/`, `fresh/` (each holds a site build and its own `target/ux`).
- Logs and outputs: `*.log`, `full1.log`, `full2.log`, `metrics1.json`, `metrics2.json`, `metrics-keep.json`, `twice.done`, `rust-cases.log`.

`cargo test` also built into the worktree's own `target/`.

## 7. Findings block

```findings
[
  {"file": "ux/lib/layout.ts", "line": 42, "category": "acceptance", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "every book element sits inside #mdbook-content (overflow auto), body-container (clip) or body (hidden), so the overflow gate never fires on a book page and a 2400 px element that scrolls the reading column sideways passes L2 at both gating viewports"},
  {"file": "ux/lib/layout.ts", "line": 152, "category": "mutant", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "the always-visible gate passes a control with opacity 0, so hiding the theme toggle that way is not caught"},
  {"file": "ux/lib/layout.ts", "line": 170, "category": "boundary", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "\"uncovered\" is tested at the centre point only, so a control covered everywhere but its centre passes the 100 % rule"},
  {"file": "ux/lib/bot.ts", "line": 78, "category": "acceptance", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "the screen-only user treats an opacity-0 link as visible and completes a task through it"},
  {"file": "crates/ignite/tests/site/ux.rs", "line": 184, "category": "mutant", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "the comparison with main's baseline checks only value, so raising an entry's tolerance lets a worse run pass with no decision"},
  {"file": "crates/ignite/tests/site/ux.rs", "line": 510, "category": "acceptance", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "the findings check accepts any existing path as the regression asset of a closed S1 or S2 finding, README.md included, without checking that the named test exists"},
  {"file": "Taskfile.yml", "line": 144, "category": "acceptance", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "in a new worktree without ux/node_modules, task gate runs npm ci against registry.npmjs.org, so task ux is not hermetic"}
]
```
