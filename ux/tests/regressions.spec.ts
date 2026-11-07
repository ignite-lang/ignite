import AxeBuilder from "@axe-core/playwright";
import { type Browser, expect, test } from "@playwright/test";
import { themes, type Theme } from "../adapter";
import { play } from "../lib/bot";
import { chooseTheme, hermetic, open } from "../lib/browse";
import { overflow, truncation } from "../lib/layout";
import { tasks } from "../lib/pages";

// One regression asset per closed finding (ux/findings/UXF-nnn.md): each
// test failed on the site before the finding's fix and passes after it.
// A finding's `regression_asset` names its test by title.

async function page(browser: Browser, path: string, width: number, height: number, theme: Theme = themes[0]) {
  const context = await browser.newContext({ viewport: { width, height }, reducedMotion: "reduce" });
  await hermetic(context, []);
  await chooseTheme(context, theme);
  const tab = await context.newPage();
  await open(tab, path);
  return { context, tab };
}

async function axeNodes(browser: Browser, path: string, width: number, height: number, rules: string[], theme?: Theme) {
  const { context, tab } = await page(browser, path, width, height, theme);
  const result = await new AxeBuilder({ page: tab }).withRules(rules).analyze();
  await context.close();
  return result.violations.flatMap((v) => v.nodes.map((n) => `${v.id}: ${n.target.join(" ")}`));
}

test("UXF-001: scrolling code blocks and tables take focus", async ({ browser }) => {
  for (const path of ["index.html", "book/reference/index.html", "book/design/intent.html"]) {
    expect(await axeNodes(browser, path, 390, 844, ["scrollable-region-focusable"]), path).toEqual([]);
  }
});

test("UXF-002: the table-of-contents toggle is a keyboard button", async ({ browser }) => {
  expect(await axeNodes(browser, "book/index.html", 390, 844, ["aria-allowed-attr", "aria-prohibited-attr"])).toEqual([]);
  const { context, tab } = await page(browser, "book/index.html", 390, 844);
  await tab.getByRole("button", { name: "Toggle Table of Contents" }).focus();
  await tab.keyboard.press("Enter");
  const open = await tab.evaluate(() => document.documentElement.classList.contains("sidebar-visible"));
  await context.close();
  expect(open, "Enter on the focused toggle opens the table of contents").toBe(true);
});

test("UXF-003: sidebar and menu-bar targets are at least 24 px", async ({ browser }) => {
  expect(await axeNodes(browser, "book/design/intent.html", 1440, 900, ["target-size"])).toEqual([]);
});

test("UXF-004: accent text on the light landing page has 4.5:1 contrast", async ({ browser }) => {
  const light = themes.find((theme) => theme.name === "light")!;
  expect(await axeNodes(browser, "index.html", 1440, 900, ["color-contrast"], light)).toEqual([]);
});

test("UXF-005: the landing page does not scroll sideways", async ({ browser }) => {
  for (const [width, height] of [
    [390, 844],
    [1440, 900],
  ]) {
    const { context, tab } = await page(browser, "index.html", width, height);
    const problems = await overflow(tab);
    await context.close();
    expect(problems, `${width}x${height}`).toEqual([]);
  }
});

test("UXF-006: window titles keep their full text", async ({ browser }) => {
  const { context, tab } = await page(browser, "index.html", 390, 844);
  const problems = await truncation(tab);
  await context.close();
  expect(problems).toEqual([]);
});

test("UXF-007: the std.fs reference page is found from the landing page", async ({ browser }) => {
  const task = tasks().find((t) => t.id === "std-fs")!;
  for (const mode of ["pointer", "keyboard"] as const) {
    const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, reducedMotion: "reduce" });
    await hermetic(context, []);
    const tab = await context.newPage();
    try {
      await play(tab, task, mode);
    } finally {
      await context.close();
    }
  }
});

test("UXF-008: the FAQ is reachable from the landing page's main navigation", async ({ browser }) => {
  const task = tasks().find((t) => t.id === "faq-platforms")!;
  for (const [width, height] of [
    [1440, 900],
    [1280, 800],
    [1024, 768],
    [768, 1024],
    [390, 844],
  ]) {
    const { context, tab } = await page(browser, "index.html", width, height);
    const run = await play(tab, task, "pointer").finally(() => context.close());
    const step = run.steps.find((s) => s.label.includes("FAQ"))!;
    expect(step.actions, `${width}x${height}: ${run.log.join(", ")}`).toBeLessThanOrEqual(2);
  }
});
