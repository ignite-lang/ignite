import { expect, test } from "@playwright/test";
import { gatingViewports, measuredViewports } from "../adapter";
import { newRun, play } from "../lib/bot";
import { baseUrl } from "../lib/browse";
import type { Card } from "../lib/cards";
import { thumbWidth } from "../lib/contact";
import { tasks } from "../lib/pages";
import { sessionContext } from "../lib/session";

// The round's tools (ux/round/, review-report R-01 section 3): the trial
// keeps the theme a session chose, a dark card runs dark, the keyboard
// user's step has a frame with the focus on the control, and a contact
// sheet stays readable.

const desktop = gatingViewports[0];

function card(theme: "light" | "dark", mode: "pointer" | "keyboard" = "pointer"): Card {
  return {
    id: `tools-${theme}`,
    kind: "anchor",
    theme,
    persona: "a test",
    facts: "",
    mode,
    tasks: [],
    between: [],
    twists: [],
  };
}

async function shown(page: import("@playwright/test").Page) {
  return page.evaluate(() => ({
    stored: localStorage.getItem("mdbook-theme"),
    landing: document.documentElement.dataset.theme ?? null,
    coal: document.documentElement.classList.contains("coal"),
  }));
}

test("T1: a session keeps a theme switched on the landing page across page loads", async ({ browser }) => {
  const context = await sessionContext(browser, card("light"), desktop, []);
  const page = await context.newPage();
  await page.goto(baseUrl() + "index.html", { waitUntil: "load" });
  await page.getByRole("button", { name: "Switch between dark and light theme", exact: true }).click();
  await page.goto(baseUrl() + "book/guide/install.html", { waitUntil: "load" });
  const book = await shown(page);
  await context.close();
  expect(book).toEqual({ stored: "coal", landing: null, coal: true });
});

test("T1: a dark card's session starts dark", async ({ browser }) => {
  const context = await sessionContext(browser, card("dark"), desktop, []);
  const page = await context.newPage();
  await page.goto(baseUrl() + "index.html", { waitUntil: "load" });
  const landing = await shown(page);
  await page.goto(baseUrl() + "book/faq.html", { waitUntil: "load" });
  const book = await shown(page);
  await context.close();
  expect(landing.landing).toBe("dark");
  expect(book.coal).toBe(true);
});

test("T6: the keyboard user's step has a frame with the focus on the control", async ({ browser }) => {
  const context = await sessionContext(browser, card("light", "keyboard"), desktop, []);
  const page = await context.newPage();
  const labels: string[] = [];
  const run = newRun(async (label) => {
    labels.push(label);
  });
  const install = tasks().find((task) => task.id === "install")!;
  await play(page, install, "keyboard", { run });
  await context.close();
  const step = run.steps[0] as { frame: number; found?: number };
  expect(step.found, JSON.stringify(run.steps)).toBeGreaterThan(step.frame);
  expect(labels[(step.found ?? 0) - 1]).toMatch(/^focus on link/);
});

test("T8: a contact-sheet tile is at least half its viewport wide", () => {
  for (const viewport of [...gatingViewports, ...measuredViewports]) {
    expect(thumbWidth(viewport), viewport.name).toBeGreaterThanOrEqual(viewport.width / 2);
  }
});
