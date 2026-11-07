import { expect, test } from "@playwright/test";
import { alwaysVisible, motion, overflow, truncation } from "../lib/layout";
import { hermetic, open } from "../lib/browse";

// Break-tests of the layout contract: each check fires on a page built to
// break it and stays quiet on its fixed twin, so a green L2 run means the
// checks ran, not that they cannot fail.

test.use({ viewport: { width: 400, height: 300 } });

test("overflow: a wide line breaks the page, a scroll box holds it", async ({ page }) => {
  await page.setContent(`<body style="margin:0"><div style="width:300px"><p style="white-space:nowrap">${"x".repeat(200)}</p></div></body>`);
  const broken = await overflow(page);
  const checks = broken.map((p) => p.check);
  expect(checks).toContain("horizontal page scroll");
  expect(broken.filter((p) => p.check === "element past its box").map((p) => p.detail.split(" ")[0])).toContain("p");
  await page.setContent(`<body style="margin:0"><div style="width:300px;overflow-x:auto"><p style="white-space:nowrap">${"x".repeat(200)}</p></div></body>`);
  expect((await overflow(page)).map((p) => p.check), "a sideways scroller that is no scroll region").toEqual(["horizontal page scroll"]);
  await page.setContent(`<body style="margin:0"><pre style="width:300px;overflow-x:auto">${"x".repeat(200)}</pre></body>`);
  expect(await overflow(page)).toEqual([]);
  await page.setContent(`<body style="margin:0"><div style="width:300px;overflow:hidden"><p style="white-space:nowrap">${"x".repeat(200)}</p></div></body>`);
  expect((await overflow(page)).map((p) => p.check), "text cut off by a clipping box").toEqual(["element past its box"]);
});

test("truncation: an ellipsis needs the full text in title", async ({ page }) => {
  const text = "a long title that cannot fit into the box";
  const style = "display:block;width:80px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis";
  await page.setContent(`<span style="${style}">${text}</span>`);
  expect((await truncation(page)).map((p) => p.check)).toEqual(["truncated text"]);
  await page.setContent(`<span style="${style}" title="${text}">${text}</span>`);
  expect(await truncation(page)).toEqual([]);
  await page.setContent(`<div style="width:80px;overflow:hidden"><pre style="overflow:auto;margin:0">${text}</pre></div>`);
  expect(await truncation(page)).toEqual([]);
});

test("always visible: out of view and covered controls are refused", async ({ page }) => {
  const controls = [{ label: "bar", selector: "#bar" }];
  await page.setContent(`<div id="bar" style="position:absolute;top:280px;height:40px;width:100px">bar</div>`);
  expect((await alwaysVisible(page, controls, 400))[0].detail).toContain("not fully in view");
  await page.setContent(`<div id="bar" style="height:40px;width:100px">bar</div><div style="position:fixed;top:0;left:0;width:200px;height:60px;background:#000"></div>`);
  expect((await alwaysVisible(page, controls, 400))[0].detail).toContain("covered at its centre");
  await page.setContent(`<div style="height:20px;overflow:hidden"><div id="bar" style="height:40px;width:100px">bar</div></div>`);
  expect((await alwaysVisible(page, controls, 400))[0].detail).toContain("clipped");
  await page.setContent(`<body style="margin:0"><div id="bar" style="height:40px;width:100px">bar</div><div style="position:fixed;top:0;left:98px;width:5px;height:60px;background:#000"></div></body>`);
  expect((await alwaysVisible(page, controls, 400))[0].detail, "a 1 px cover at the edge").toContain("covered at");
  await page.setContent(`<div id="bar" style="height:40px;width:100px;border-radius:20px;background:#ccc">bar</div>`);
  expect(await alwaysVisible(page, controls, 400), "rounded corners are no cover").toEqual([]);
  await page.setContent(`<div id="bar" style="height:40px;width:100px">bar</div>`);
  expect(await alwaysVisible(page, controls, 400)).toEqual([]);
  expect(await alwaysVisible(page, [{ label: "wide only", selector: "#none", when: (w) => w > 1000 }], 400)).toEqual([]);
});

test("reduced motion: a running animation is refused", async ({ page }) => {
  const css = "@keyframes spin{to{transform:rotate(1turn)}} .s{animation:spin 1s infinite}";
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.setContent(`<style>${css}</style><div class="s">x</div>`);
  expect((await motion(page)).map((p) => p.check)).toEqual(["reduced motion"]);
  await page.setContent(`<style>${css} @media (prefers-reduced-motion: reduce){.s{animation:none}}</style><div class="s">x</div>`);
  expect(await motion(page)).toEqual([]);
});

test("overflow: a 2400 px block on a built book page is refused at both gating widths", async ({ browser }) => {
  for (const [width, height] of [
    [1440, 900],
    [390, 844],
  ]) {
    const context = await browser.newContext({ viewport: { width, height } });
    await hermetic(context, []);
    const page = await context.newPage();
    await open(page, "book/faq.html");
    expect(await overflow(page), `book/faq.html at ${width}x${height} before the block`).toEqual([]);
    await page.evaluate(() => {
      const block = document.createElement("div");
      block.style.width = "2400px";
      block.textContent = "wide";
      document.querySelector("main")!.prepend(block);
    });
    const problems = await overflow(page);
    await context.close();
    expect(problems.map((p) => p.check), `${width}x${height}`).toContain("horizontal page scroll");
  }
});
