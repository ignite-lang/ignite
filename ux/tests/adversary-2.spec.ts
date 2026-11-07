import { expect, test } from "@playwright/test";
import { alwaysVisible as controls } from "../adapter";
import { play } from "../lib/bot";
import { baseUrl, hermetic, open } from "../lib/browse";
import { alwaysVisible, overflow } from "../lib/layout";
import type { Task } from "../lib/pages";

// Adversary cases, pass 2 (story docs-site-ui-loop, part 1, against
// 83f673dd): each asserts what the gate claims and is red when the gate
// cannot see the break.

test.describe("adversary 2: always visible, 100 % uncovered", () => {
  test("a fixed bar over the bottom 100 px of the book's sidebar at 1440 is refused", async ({ browser }) => {
    const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
    await hermetic(context, []);
    const page = await context.newPage();
    await open(page, "book/faq.html");
    expect(await alwaysVisible(page, controls.book, 1440), "before the bar").toEqual([]);
    await page.evaluate(() => {
      const bar = document.createElement("div");
      bar.style.cssText = "position:fixed;left:0;right:0;bottom:0;height:100px;background:#000;z-index:2147483647";
      document.body.append(bar);
    });
    const problems = await alwaysVisible(page, controls.book, 1440);
    await context.close();
    expect(problems.map((p) => p.detail).join("\n"), "the sidebar's bottom 100 px are covered").toContain("sidebar");
  });

  test("a box over the first 10 % of the landing page's navigation at 1440 is refused", async ({ browser }) => {
    const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
    await hermetic(context, []);
    const page = await context.newPage();
    await open(page, "index.html");
    expect(await alwaysVisible(page, controls.landing, 1440), "before the cover").toEqual([]);
    await page.evaluate(() => {
      const nav = document.querySelector("#main-nav")!.getBoundingClientRect();
      const cover = document.createElement("div");
      cover.style.cssText = `position:fixed;left:${nav.left}px;top:${nav.top}px;width:${nav.width * 0.1}px;height:${nav.height}px;background:#000;z-index:2147483647`;
      document.body.append(cover);
    });
    const problems = await alwaysVisible(page, controls.landing, 1440);
    await context.close();
    expect(problems.map((p) => p.detail).join("\n"), "the navigation's first 10 % are covered").toContain("main navigation");
  });
});

test.describe("adversary 2: overflow", () => {
  for (const [width, height] of [
    [1440, 900],
    [390, 844],
  ]) {
    test(`text pushed past the left edge of the book's reading column is refused at ${width}x${height}`, async ({ browser }) => {
      const context = await browser.newContext({ viewport: { width, height } });
      await hermetic(context, []);
      const page = await context.newPage();
      await open(page, "book/faq.html");
      expect(await overflow(page), "before the paragraph").toEqual([]);
      await page.evaluate(() => {
        const p = document.createElement("p");
        p.style.marginLeft = "-2000px";
        p.textContent = "This sentence starts far left of the reading column and no reader can scroll to it.";
        document.querySelector("main")!.prepend(p);
      });
      const problems = await overflow(page);
      await context.close();
      expect(problems.length, "a paragraph cut off at the column's left edge passes the overflow gate").toBeGreaterThan(0);
    });
  }
});

test.describe("adversary 2: screen-only user", () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test("keyboard: does not press Enter on a link outside the viewport", async ({ page }) => {
    const base = baseUrl();
    await page.route(base + "adversary2-start.html", (route) =>
      route.fulfill({
        contentType: "text/html",
        body: `<main><a href="adversary2-goal.html" style="position:absolute;left:-9999px">Goal</a></main>`,
      }),
    );
    await page.route(base + "adversary2-goal.html", (route) =>
      route.fulfill({ contentType: "text/html", body: `<main>the goal</main>` }),
    );
    const task: Task = {
      id: "adversary2-offscreen",
      goal: "reach the goal through a link outside the viewport",
      start: "adversary2-start.html",
      truth: { path: "adversary2-goal.html", text: "the goal" },
      route: [{ click: { role: "link", name: "^Goal$" } }],
    };
    await expect(play(page, task, "keyboard")).rejects.toThrow();
  });
});
