import { expect, test } from "@playwright/test";
import { gatingViewports, OUT_DIR, themes } from "../adapter";
import { nearestRank, play } from "../lib/bot";
import { chooseTheme, hermetic } from "../lib/browse";
import { tasks, writeRecord } from "../lib/pages";

for (const viewport of gatingViewports) {
  test.describe(`L3 ${viewport.name}`, () => {
    for (const task of tasks()) {
      test(task.id, async ({ browser }) => {
        const results = [];
        for (const mode of ["pointer", "keyboard"] as const) {
          const refused: string[] = [];
          const context = await browser.newContext({
            viewport: { width: viewport.width, height: viewport.height },
            reducedMotion: "reduce",
          });
          await hermetic(context, refused);
          await chooseTheme(context, themes[0]);
          const page = await context.newPage();
          try {
            results.push(await play(page, task, mode));
          } finally {
            await context.close();
          }
          expect(refused, `${task.id} (${mode}) requested a resource off the local server`).toEqual([]);
        }
        const [pointer, keyboard] = results;
        writeRecord(OUT_DIR, `l3-${viewport.name}-${task.id}`, {
          group: "gating",
          scenario: `${task.id}/${viewport.name}`,
          metrics: {
            "screen_only.actions": pointer.actions,
            "screen_only.looks": pointer.looks,
            "screen_only.steers": pointer.steers,
            "screen_only.time_to_act_steps.max": Math.max(0, ...pointer.timeToAct),
            "glance_cost.median": nearestRank(pointer.glances, 0.5),
            "glance_cost.p95": nearestRank(pointer.glances, 0.95),
            "keyboard.actions": keyboard.actions,
          },
        });
        test.info().annotations.push({ type: "pointer", description: pointer.log.join(", ") });
        test.info().annotations.push({ type: "keyboard", description: keyboard.log.join(", ") });
      });
    }
  });
}
