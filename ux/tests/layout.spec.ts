import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";
import {
  alwaysVisible as controls,
  gatingViewports,
  measuredViewports,
  OUT_DIR,
  pageKind,
  themes,
  type Viewport,
} from "../adapter";
import { chooseTheme, hermetic, open } from "../lib/browse";
import { alwaysVisible, motion, overflow, type Problem, truncation } from "../lib/layout";
import { missedShot, sitePages, writeRecord } from "../lib/pages";

const AXE_TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"];

const pages = sitePages();

type Counts = Record<string, number>;

function format(problems: Problem[]): string {
  return problems.map((problem) => `  ${problem.check}: ${problem.detail}`).join("\n");
}

function run(viewport: Viewport, gating: boolean) {
  test.describe(`L2 ${viewport.name}${gating ? "" : " (measured only)"}`, () => {
    for (const path of pages) {
      test(path, async ({ browser }) => {
        const kind = pageKind(path);
        const counts: Counts = {
          "layout.overflow": 0,
          "layout.truncated": 0,
          "always_visible.misses": 0,
          "external_requests": 0,
        };
        if (gating) {
          counts["axe.serious_critical"] = 0;
          counts["reduced_motion.violations"] = 0;
        }
        const failures: string[] = [];
        for (const theme of themes) {
          const refused: string[] = [];
          const context = await browser.newContext({
            viewport: { width: viewport.width, height: viewport.height },
            reducedMotion: "no-preference",
          });
          await hermetic(context, refused);
          await chooseTheme(context, theme);
          const page = await context.newPage();
          await open(page, path);
          const flows = await overflow(page);
          const cuts = await truncation(page);
          const hidden = await alwaysVisible(page, controls[kind], viewport.width);
          counts["layout.overflow"] += flows.length;
          counts["layout.truncated"] += cuts.length;
          counts["always_visible.misses"] += hidden.length;
          const problems = [...flows, ...cuts, ...hidden];
          let axeNodes = 0;
          if (gating) {
            const axe = await new AxeBuilder({ page }).withTags(AXE_TAGS).analyze();
            for (const violation of axe.violations) {
              if (violation.impact === "serious" || violation.impact === "critical") {
                counts["axe.serious_critical"] += violation.nodes.length;
                axeNodes += violation.nodes.length;
                for (const node of violation.nodes) {
                  problems.push({
                    check: `axe ${violation.id} (${violation.impact})`,
                    detail: `${node.target.join(" ")}: ${node.failureSummary?.split("\n").slice(1).join(" ").trim() ?? ""}`,
                  });
                }
              }
            }
          }
          const missed: [string, number][] = [
            ["layout.overflow", flows.length],
            ["layout.truncated", cuts.length],
            ["always_visible.misses", hidden.length],
            ["axe.serious_critical", axeNodes],
            ["external_requests", refused.length],
          ];
          for (const [bar, count] of missed) {
            if (count) {
              await missedShot(page, viewport.name, bar, `${theme.name}-${path}`);
            }
          }
          await context.close();
          counts["external_requests"] += refused.length;
          for (const url of refused) {
            problems.push({ check: "external request", detail: url });
          }
          if (problems.length) {
            failures.push(`${theme.name}:\n${format(problems)}`);
          }
        }
        if (gating) {
          const context = await browser.newContext({
            viewport: { width: viewport.width, height: viewport.height },
            reducedMotion: "reduce",
          });
          const refused: string[] = [];
          await hermetic(context, refused);
          await chooseTheme(context, themes[0]);
          const page = await context.newPage();
          await open(page, path);
          const moving = await motion(page);
          if (moving.length) {
            await missedShot(page, viewport.name, "reduced_motion.violations", path);
          }
          await context.close();
          counts["reduced_motion.violations"] += moving.length;
          if (moving.length) {
            failures.push(`reduced motion:\n${format(moving)}`);
          }
        }
        writeRecord(OUT_DIR, `l2-${viewport.name}-${path}`, {
          group: gating ? "gating" : "measured",
          scenario: `pages/${viewport.name}`,
          metrics: counts,
        });
        if (gating) {
          expect(failures, `${path} at ${viewport.name}:\n${failures.join("\n")}`).toEqual([]);
        }
      });
    }
  });
}

for (const viewport of gatingViewports) {
  run(viewport, true);
}
for (const viewport of measuredViewports) {
  run(viewport, false);
}
