import { expect, test } from "@playwright/test";
import { alwaysVisible as controls } from "../adapter";
import { play } from "../lib/bot";
import { baseUrl, hermetic, open, settle } from "../lib/browse";
import { alwaysVisible, overflow } from "../lib/layout";
import type { Task } from "../lib/pages";

// Adversary cases (story docs-site-ui-loop, part 1): each asserts what the
// gate claims and is red when the gate cannot see the break.

test.describe("adversary: layout contract", () => {
  test.use({ viewport: { width: 400, height: 300 } });

  test("always visible refuses a control a reader cannot see (opacity 0)", async ({ page }) => {
    await page.setContent(`<div id="bar" style="height:40px;width:100px;opacity:0">bar</div>`);
    const problems = await alwaysVisible(page, [{ label: "bar", selector: "#bar" }], 400);
    expect(problems.map((p) => p.check)).toEqual(["always visible"]);
  });

  test("always visible refuses a control covered everywhere but its centre", async ({ page }) => {
    // Two opaque boxes cover the left and right 45 % of the control; only a
    // 10 px strip at its centre shows.
    await page.setContent(
      `<div id="bar" style="position:absolute;top:0;left:0;height:40px;width:100px">bar</div>` +
        `<div style="position:fixed;top:0;left:0;width:45px;height:60px;background:#000"></div>` +
        `<div style="position:fixed;top:0;left:55px;width:45px;height:60px;background:#000"></div>`,
    );
    const problems = await alwaysVisible(page, [{ label: "bar", selector: "#bar" }], 400);
    expect(problems.map((p) => p.check)).toEqual(["always visible"]);
  });

  test("overflow: a wide element in mdBook's scrolling content column is refused", async ({ page }) => {
    // The book's structure, as built: body overflow-x hidden,
    // #mdbook-body-container overflow-x clip, #mdbook-content overflow auto
    // around <main>. A 2400 px element there scrolls the whole reading
    // column sideways.
    await page.setContent(
      `<body style="margin:0;overflow-x:hidden"><div style="overflow-x:clip"><div class="page">` +
        `<div id="mdbook-content" class="content" style="overflow:auto;height:300px"><main>` +
        `<div style="width:2400px">wide</div><p>text</p></main></div></div></div></body>`,
    );
    const problems = await overflow(page);
    expect(problems.length, "no problem reported for a 2400 px element in a 400 px reading column").toBeGreaterThan(0);
  });
});

test.describe("adversary: screen-only user", () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test("does not click a link it cannot see (opacity 0)", async ({ page }) => {
    const base = baseUrl();
    await page.route(base + "adversary-start.html", (route) =>
      route.fulfill({
        contentType: "text/html",
        body: `<main><a href="adversary-goal.html" style="opacity:0">Goal</a></main>`,
      }),
    );
    await page.route(base + "adversary-goal.html", (route) =>
      route.fulfill({ contentType: "text/html", body: `<main>the goal</main>` }),
    );
    const task: Task = {
      id: "adversary-invisible",
      goal: "reach the goal through a link nobody can see",
      start: "adversary-start.html",
      truth: { path: "adversary-goal.html", text: "the goal" },
      route: [{ click: { role: "link", name: "^Goal$" } }],
    };
    await expect(play(page, task, "pointer")).rejects.toThrow(/no usable link/);
  });

  test("keyboard: every Enter lands on a focused control inside the viewport (390x844)", async ({ browser }) => {
    const context = await browser.newContext({ viewport: { width: 390, height: 844 }, reducedMotion: "reduce" });
    const refused: string[] = [];
    await hermetic(context, refused);
    const enters: string[] = [];
    await context.exposeBinding("adversaryEnter", (_source, what: string) => {
      enters.push(what);
    });
    await context.addInitScript(() => {
      addEventListener(
        "keydown",
        (event) => {
          if (event.key !== "Enter") {
            return;
          }
          const el = document.activeElement as HTMLElement | null;
          const rect = el ? el.getBoundingClientRect() : null;
          const inView =
            !!rect && rect.width > 0 && rect.left >= 0 && rect.top >= 0 && rect.right <= innerWidth && rect.bottom <= innerHeight;
          (window as unknown as { adversaryEnter: (what: string) => void }).adversaryEnter(
            `${inView ? "in view" : "OUT OF VIEW"} ${el?.tagName} "${(el?.textContent ?? "").trim().slice(0, 30)}" ${rect ? Math.round(rect.left) + "," + Math.round(rect.top) : ""}`,
          );
        },
        true,
      );
    });
    const page = await context.newPage();
    const task: Task = {
      id: "http-example",
      goal: "Find an example of an HTTP server.",
      start: "index.html",
      truth: { path: "book/reference/std/http.html", text: "A server in a system" },
      route: [
        { click: { role: "link", name: "^Reference$" } },
        { click: { role: "link", name: "^Standard library$" } },
        { click: { role: "link", name: "^std\\.protocol\\.http$" } },
      ],
    };
    const run = await play(page, task, "keyboard");
    await context.close();
    expect(enters.length, run.log.join(", ")).toBeGreaterThan(0);
    expect(enters.filter((e) => !e.startsWith("in view")), `${run.log.join(", ")}\n${enters.join("\n")}`).toEqual([]);
  });
});

test.describe("adversary: always visible after reading", () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test("book: the menu bar, theme and search toggles stay in view after scrolling down", async ({ browser }) => {
    const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, reducedMotion: "reduce" });
    const refused: string[] = [];
    await hermetic(context, refused);
    const page = await context.newPage();
    await open(page, "book/reference/diagnostics.html");
    await page.mouse.move(800, 450);
    for (let i = 0; i < 3; i++) {
      await page.mouse.wheel(0, 700);
      await page.waitForTimeout(100);
    }
    await settle(page);
    await page.waitForTimeout(300);
    const problems = await alwaysVisible(page, controls.book, 1440);
    await context.close();
    expect(problems).toEqual([]);
  });
});
