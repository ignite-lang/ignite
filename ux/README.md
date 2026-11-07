# The website's UI loop

The public website (`site/`, built by `task site` into `target/site/`) runs the measured UI
improvement loop: every merge checks layout, accessibility and a screen-only user against a
ratcheted baseline, and every finding becomes a regression test.

## Run it

Once, after a clone (installs the pinned Playwright and axe-core from `package-lock.json`;
no browser download, Playwright uses the Chromium in `~/.cache/ms-playwright`):

```sh
cd ux && PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1 npm ci
```

`task ux` does this itself when `ux/node_modules` is missing and never touches the network
when it exists. Playwright 1.65.0 needs Chromium revision 1254
(`npx playwright install chromium` fetches it when it is missing).

Then:

```sh
task ux
```

It builds the site, serves it with `miniserve` on a free loopback port, runs the Playwright
specs and then the ratchet. `task gate` runs it.

## What it checks

| level | file | checks |
|---|---|---|
| L2 layout contract | `tests/layout.spec.ts` | every page of the site at the gating viewports and in every theme: no horizontal page scroll, no element past its box, the always-visible controls fully in view and uncovered at their centre, axe-core 0 serious or critical violations (`wcag2a`, `wcag2aa`, `wcag21a`, `wcag21aa`, `wcag22aa`), no truncated text without its full value, no motion under `prefers-reduced-motion: reduce`, no request off the local server |
| L3 screen-only user | `tests/screen-only.spec.ts`, `lib/bot.ts` | every task under `scenarios/`, from the landing page, with the pointer and with the keyboard alone |
| ratchet | `crates/ignite/tests/site/ux.rs` (`site::ux::`) | `target/ux/metrics.json` against `baseline.json` |
| findings | same | a closed S1 or S2 finding under `findings/` names its regression asset |

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
