# The website's UI loop

The public website (`site/`, built by `task site` into `target/site/`) runs the measured UI
improvement loop: every merge checks layout, accessibility and a screen-only user against a
ratcheted baseline, and every finding becomes a regression test.

## Run it

`task ux` never touches the network. Its `ux-deps` step links `ux/node_modules` to one
shared install per lockfile, `$XDG_CACHE_HOME/ignite/ux/<hash>/node_modules` (default
`~/.cache`; `<hash>` the first 16 hex digits of the SHA-256 of `package-lock.json`), so every
worktree uses the same copy. When that copy is missing it is made with
`npm ci --offline` from the npm cache. When the npm cache does not hold the packages either,
the step fails with one line naming the one-time command, run once per machine and lockfile,
with network:

```sh
task ux-install
```

A `ux/node_modules` that is a real directory (a manual `npm ci` in `ux/`) is used as it is.
No browser is downloaded: Playwright 1.65.0 uses Chromium revision 1254 in
`~/.cache/ms-playwright` (`cd ux && npx playwright install chromium` fetches it, with
network, when it is missing).

Then:

```sh
task ux
```

It builds the site, serves it with `miniserve` on a free loopback port, runs the Playwright
specs and then the ratchet. `task gate` runs it.

## What it checks

| level | file | checks |
|---|---|---|
| L2 layout contract | `tests/layout.spec.ts` | every page of the site at the gating viewports and in every theme: no horizontal page scroll (the document, or any box that scrolls sideways and is not one of `adapter.ts`'s `scrollRegions`: on a book page the reading column), no text or control cut off or past its box, the always-visible controls opaque, fully in view and uncovered at their centre and every point of a 5 by 5 grid from 1 px inside their edges (only points outside a rounded corner skipped), axe-core 0 serious or critical violations (`wcag2a`, `wcag2aa`, `wcag21a`, `wcag21aa`, `wcag22aa`), no truncated text without its full value, no motion under `prefers-reduced-motion: reduce`, no request off the local server |
| L3 screen-only user | `tests/screen-only.spec.ts`, `lib/bot.ts` | every task under `scenarios/`, from the landing page, with the pointer and with the keyboard alone |
| ratchet | `crates/ignite/tests/site/ux.rs` (`site::ux::`) | `target/ux/metrics.json` against `baseline.json` |
| findings | same | a closed S1 or S2 finding under `findings/` names its regression asset as `<path> :: <test name>`, a relative path under `ux/` or `crates/` whose file holds that test; `tests/regressions.spec.ts` holds one test per finding |
| break-tests | `tests/contract.spec.ts`, `tests/adversary.spec.ts` | each check of L2 and the screen-only user fires on a page built to break it |

The adapter (viewports, themes, always-visible controls, navigation disclosures) is
`adapter.ts`. Measured-only viewports are reported at the end of the Playwright run and in
`target/ux/metrics.json` under `measured`, and never gated.

## The ratchet

Every value is lower-is-better. The ratchet fails on a value worse than `baseline.json`
beyond its tolerance, on a value missing from the run, on a value the baseline does not
hold, and on an improvement not written into the baseline. After an improvement:

```sh
IGNITE_BLESS=1 cargo test -p ignite --test main -- --ignored --exact site::ux::ratchet
```

writes the improved and the new values. A worse value is never blessed: raise it by hand
with a `decision` naming the round's decision, or the ratchet refuses the baseline against
`main`'s.

## Tasks

A task is `scenarios/<id>/task.json`: the goal, the start page, the truth (the page and a
text it shows) and the route, the accessible role and name of each control a reader uses.
The screen-only user scrolls or opens the menu when a control is not in view, and counts
every click, wheel turn and key press.

## Findings

`findings/UXF-nnn.md`, a `---` header with `id`, `title`, `class`, `severity`, `metric`,
`status`, `found_by`, `evidence_recording`, `evidence_screenshot`, `evidence_lines` and
`regression_asset` (a path, then what in it), and the evidence below it.
