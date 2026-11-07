import { expect, test } from "@playwright/test";
import { alwaysVisible, motion, overflow, truncation } from "../lib/layout";

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
  expect(await overflow(page)).toEqual([]);
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
