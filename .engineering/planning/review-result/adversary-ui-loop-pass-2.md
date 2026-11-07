---
format: aep.planning-md/3
id: review-result:adversary-ui-loop-pass-2
kind: review-result
status: active
title: Adversary, docs-site-ui-loop, pass 2
relations:
- reviews: story:docs-site-ui-loop
revision: 1
---
unit: story:docs-site-ui-loop part 1, second adversary pass, worktree ui-loop at 83f673dd plus my two uncommitted test files
verdict: CONFIRMED (4 warnings, 1 note; none blocks the merge alone)
cases: executed 274→282 (Playwright 267→272, `site::ux::` 7→10), red 8
origin: introduced 5 / pre-existing 0 / undecided 0
wrote-outside-worktree: 1 directory tree (the assigned scratch directory adversary2/, listed in part 6)
needs-coordinator: I ran `task ux` from a fresh export (it passes) but not the full `task gate` (check and test take about 25 min and the correction does not touch them)

## 1. Diff

```
 crates/ignite/tests/site/ux.rs | 51 ++++++++++++++++++++++++++++++++++++++++++
 1 file changed, 51 insertions(+)
?? ux/tests/adversary-2.spec.ts   (new, untracked)
```

Both are test files. I edited no implementation file and changed no existing case. All mutations and probes ran on scratch copies.

## 2. Cases added (red output from running them alone, before any suite run)

`ux/tests/adversary-2.spec.ts`, run alone: 5 failed, exit 1.

1. **A fixed bar over the bottom 100 px of the book sidebar, `book/faq.html` at 1440.** The page is clean before the bar is added.
   - Red: `Error: the sidebar's bottom 100 px are covered … Expected substring: "sidebar"  Received string: ""`
2. **A box over the first 10 % of `#main-nav`, `index.html` at 1440.**
   - Red: `Error: the navigation's first 10 % are covered … Expected substring: "main navigation"  Received string: ""`
3. **Text pushed past the left edge of the reading column (`margin-left:-2000px`), `book/faq.html` at 1440.**
   - Red: `Error: a paragraph cut off at the column's left edge passes the overflow gate … Expected: > 0  Received: 0`
4. **The same at 390.** Same red output.
5. **Keyboard mode: Enter on a link at `left:-9999px`.**
   - Red: `Received promise resolved instead of rejected  Resolved to value: {"actions": 2, … "log": ["enter on link /^Goal$/ after 1 tabs"]}`

`crates/ignite/tests/site/ux.rs`, run alone with `cargo test -q -p ignite --test main -- site::ux::adversary2_`: 3 failed, exit 101.

6. `adversary2_a_test_name_found_only_in_a_comment_is_refused`
   - Red: `panicked at ux.rs:706:5: a regression asset naming words of a comment, not a test, passes the findings check`
7. `adversary2_a_fragment_of_test_titles_is_refused`
   - Red: `panicked at ux.rs:721:5: `UXF`, a fragment of seven test titles, passes as the name of a regression test`
8. `adversary2_a_source_file_that_is_no_test_is_refused`
   - Red: `panicked at ux.rs:737:5: crates/ignite/src/main.rs :: fn main passes as a regression asset`

## 3. Suite runs (after the cases existed)

- **Playwright, full suite on a scratch copy of HEAD's harness and site:** `npx --no-install playwright test --workers=10` gave `5 failed, 267 passed (1.9m)`, exit 1.
  - The 5 failures are cases 1 to 5.
  - All 13 pass-1 and regression cases pass.
- **The same run with my cases excluded (`--grep-invert "adversary 2"`):** `267 passed`, exit 0. This is the "before" count.
- **Rust:** `cargo test -q -p ignite --test main -- site::ux::` gave `FAILED. 7 passed; 3 failed; 1 ignored`, exit 101. The 3 failures are cases 6 to 8; the two pass-1 cases are green.
- **`task ux` in a fresh `git archive 83f673dd` export** (no `node_modules`, its own target): exit 0. `ux-deps` linked the existing shared install and the ratchet gave `1 passed`.

## 4. Findings (they cover 83f673dd)

1. **ux/lib/layout.ts:249: "uncovered" covers only the central 70 %, but the acceptance says 100 %. Warning, CONFIRMED, introduced.**
   - **Measured:** cases 1 and 2 are red. Up to 15 % of each edge goes unchecked: 135 px at each end of the sidebar and about 108 px at each end of the main navigation.
   - **What reaches it:** a fixed footer or banner, or a sticky element over the edge of a large control. No current page does this.
   - **Fix to name:** sample up to 1 px inside the edges. Exclude only round corners, for example with a radius-aware inset.
2. **ux/lib/layout.ts:71: content cut off on the left is never seen. Warning, CONFIRMED, introduced.**
   - **Measured:** cases 3 and 4 are red. A probe put the text at x −1536 to −921 against a column at 323 (1440), and at x −1980 to −1365 against 15 (390). `scrollLeft` stays 0, so no reader can reach it.
   - **Cause:** `scrollWidth` does not count overflow at the start edge, and every check, the clip check included, is skipped when `scrollWidth <= clientWidth`.
   - **What reaches it:** a negative margin, `left` or `translateX`. No current page does this.
3. **ux/lib/bot.ts:235: in keyboard mode the screen-only user presses Enter on a focused control outside the viewport. Warning, CONFIRMED, introduced.**
   - **Measured:** case 5 is red.
   - **Cause:** `focused()` checks only `activeElement` and opacity. The viewport and hit tests are applied only in pointer mode (`usable`).
   - **What reaches it:** the classic off-screen hiding pattern. The built site has none (grep for `9999px`, `sr-only` and `visually-hidden` finds only the vendored clipboard script).
4. **crates/ignite/tests/site/ux.rs:568: anyone can fake a regression asset. Warning, CONFIRMED, introduced.**
   - **Measured:** cases 6 to 8 are red. `text.contains(test)` accepts any substring of the file:
     - words from a comment;
     - `UXF`, which matches seven test titles;
     - `fn main` in a source file that holds no test.
   - The unit's own test at ux.rs:626 asserts that `ux/tests/layout.spec.ts :: L2`, a fragment, passes.
   - **What reaches it:** writing any finding record.
   - **Fix to name:** match the title inside a `test(` or `fn` declaration (`test("<name>"` or `fn <name>(`), and require the file to be a spec or a test module.
5. **Taskfile.yml:144: once the shared install is gone, `ux-deps` never restores it. Note, CONFIRMED, introduced.**
   - **Measured** (scratch, `XDG_CACHE_HOME` pointed at scratch):
     - After `rm -rf <cache>/node_modules`, `task ux-deps` prints `Task "ux-deps" is up to date`, exit 0, and the link points at nothing.
     - `npx --no-install playwright` then fails with `npx canceled due to missing packages`.
     - With `ready` removed as well, the result is the same: still up to date.
   - **Cause:** the `status` line compares only the link's target string (`readlink`), never whether the install exists.
   - **What reaches it:** deleting `~/.cache/ignite` (a cache or quota cleanup). In every worktree linked to that install, `task ux` and so `task gate` then fail without naming `task ux-install`.

## 5. Attacked and could not break

- **`scrollRegions` breadth:** `.panel`, `.tabs` and `.speed-figure` occur only on the landing page (3, 1 and 1 times) and on no book page.
  - A scroll region that is itself wider than the column does not scroll, so the reading column scrolls and is reported.
  - A wide block inside `pre` or `.table-wrapper` passes, but that is the stated allowance for code and tables. mdBook makes `.table-wrapper` only around tables.
- **The regressions spec fails without the fixes:** under HEAD's harness, all 7 fail on the base site (8ca047d4), each for its own finding's reason:
  - UXF-001: `scrollable-region-focusable`.
  - UXF-002: `aria-allowed-attr` and `aria-prohibited-attr`.
  - UXF-003: `target-size`.
  - UXF-004: `color-contrast`.
  - UXF-005: 618 px page scroll.
  - UXF-006: cut titles.
  - UXF-007: no `std.fs` page.
  - All 7 pass on HEAD. I tested with all fixes reverted together, not one at a time.
- **Concurrent first install:** 3 rounds of 6 parallel `task ux-deps` against one empty cache all exited 0 with Playwright 1.65.0 resolving. The npm logs interleave, but the result stayed consistent.
- **Writes into the shared install:** I found none during a Playwright run. No `/tmp/playwright*` appeared.
- **Ratchet:** a loosened tolerance and a change of kind are refused. Pass-1 case 7 is green.
- **Pass-1 cases:** all 8 are green.
- **Fresh worktree:** `task ux` passes.

## 6. Paths written outside the worktree

All under `/home/timo/.cache/scratch/e18a99fc-f99c-4987-93c9-e5de2080b61a/unit-ui-loop/adversary2/`, 90 MB in total:
- Scripts: `setup.sh`, `run.sh`, `rust.sh`, `suites.sh`, `fresh.sh`, `deps-race.sh`, `deps-race2.sh`.
- Scratch copies:
  - `head/` and `base/`: harness and built site.
  - `fresh/`: the HEAD export. Its 881 MB `target/` is already deleted.
  - `deps/` and `deps2/`: scratch Taskfile copies and an `xdg/` npm install.
- Logs: `pw-cases.log`, `pw-full.log`, `pw-without.log`, `reg-base.log`, `probe.log`, `rust-cases.log`, `rust-suite.log`, `fresh.log`, `deps-race.log`, `deps-race2.log`, `suites.done`, `fresh.done`.

Other writes outside the worktree:
- npm wrote debug logs to `~/.npm/_logs/` (the dangling-link probe).
- `cargo test` built into the worktree's own `target/`.

## 7. Findings block

```findings
[
  {"file": "ux/lib/layout.ts", "line": 249, "category": "acceptance", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "the uncovered check samples only the central 70 %, so a fixed bar over the sidebar's bottom 100 px or a box over the first 10 % of the landing navigation passes the 100 % rule"},
  {"file": "ux/lib/layout.ts", "line": 71, "category": "boundary", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "overflow at the start edge is not in scrollWidth, so text pushed left out of the reading column, unreachable by scrolling, passes the overflow gate at both gating viewports"},
  {"file": "ux/lib/bot.ts", "line": 235, "category": "acceptance", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "in keyboard mode the screen-only user presses Enter on a focused link outside the viewport, because focused() checks neither viewport nor hit test"},
  {"file": "crates/ignite/tests/site/ux.rs", "line": 568, "category": "acceptance", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "the regression-asset check is a substring search, so words of a comment, a fragment such as UXF, or fn main in a non-test source file pass as a named test"},
  {"file": "Taskfile.yml", "line": 144, "category": "boundary", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "ux-deps' status compares only the link target string, so once the shared install is removed it reports up to date with a dangling link and task ux fails without naming task ux-install"}
]
```
