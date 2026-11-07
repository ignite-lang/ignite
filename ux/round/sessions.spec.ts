import * as fs from "node:fs";
import * as path from "node:path";
import { type Page, test } from "@playwright/test";
import { gatingViewports, measuredViewports, themes, type Viewport } from "../adapter";
import { follow, type Mode, nearestRank, newRun, play, type Run, start } from "../lib/bot";
import { chooseTheme, hermetic } from "../lib/browse";
import { ROUND_OUT, roundCards, roundId } from "../lib/cards";
import { REPO, type Step, type Task, tasks } from "../lib/pages";

// The trial of a round (SKILL.md section 3.1) with no person: the
// screen-only user of lib/bot.ts drives one session per scenario card and
// viewport. Every moment (the start and every action that changes the
// screen) is a frame taken 0.75 s after it; every session is recorded as a
// Playwright trace (rrweb is not in the lockfile). A session that cannot
// go on is recorded as not completed, with the reason, and the test still
// passes: the round reports it, the gating suite gates it.
//
// Output under target/ux/round/: sessions/<session>.json,
// frames/<session>/<n>.jpg, traces/<session>.zip, and for the contact sheet
// contact/<viewport>.json and sheets/contact-<viewport>.jpg.

const MOMENT_DELAY_MS = 750;
const THUMB_WIDTH = 320;

const round = roundId();
const cards = roundCards(round);
const taskById = new Map(tasks().map((task) => [task.id, task]));
const viewports: { group: "gating" | "measured"; viewport: Viewport }[] = [
  ...gatingViewports.map((viewport) => ({ group: "gating" as const, viewport })),
  ...measuredViewports.map((viewport) => ({ group: "measured" as const, viewport })),
];

function out(...parts: string[]): string {
  return path.join(REPO, ROUND_OUT, ...parts);
}

function rel(file: string): string {
  return path.relative(REPO, file).split(path.sep).join("/");
}

function pathOf(page: Page): string {
  return new URL(page.url()).pathname.replace(/^\//, "");
}

// The theme the page shows: the landing page's data-theme, else the
// book's stored choice.
async function shownTheme(page: Page): Promise<string> {
  return page.evaluate(() => {
    const landing = document.documentElement.dataset.theme;
    if (landing) {
      return landing;
    }
    let stored: string | null = null;
    try {
      stored = localStorage.getItem("mdbook-theme");
    } catch {
      // storage refused
    }
    return stored === "coal" || stored === "navy" || stored === "ayu" ? "dark" : "light";
  });
}

type Frame = { n: number; label: string; page: string; theme: string; file: string };
type Segment = {
  kind: "task" | "twist" | "between";
  id: string;
  completed: boolean;
  error?: string;
  actions: number;
  looks: number;
  steers: number;
  first_step: number;
  steps: number;
};

async function segment(run: Run, segments: Segment[], kind: Segment["kind"], id: string, body: () => Promise<unknown>) {
  const before = { actions: run.actions, looks: run.looks, steers: run.steers, steps: run.steps.length };
  const record: Segment = { kind, id, completed: false, actions: 0, looks: 0, steers: 0, first_step: before.steps, steps: 0 };
  segments.push(record);
  const close = () => {
    record.actions = run.actions - before.actions;
    record.looks = run.looks - before.looks;
    record.steers = run.steers - before.steers;
    record.steps = run.steps.length - before.steps;
  };
  try {
    await body();
    record.completed = true;
  } catch (error) {
    record.error = error instanceof Error ? error.message.split("\n")[0] : String(error);
    throw error;
  } finally {
    close();
  }
}

for (const card of cards) {
  for (const { group, viewport } of viewports) {
    test(`session ${card.id} ${viewport.name}`, async ({ browser }) => {
      const session = `${card.id}-${viewport.name}`;
      const refused: string[] = [];
      const context = await browser.newContext({
        viewport: { width: viewport.width, height: viewport.height },
        reducedMotion: "reduce",
      });
      await hermetic(context, refused);
      await chooseTheme(context, themes[0]);
      await context.tracing.start({ screenshots: true, snapshots: true, title: `${round} ${session}` });
      const page = await context.newPage();
      const frameDir = out("frames", session);
      fs.mkdirSync(frameDir, { recursive: true });
      const frames: Frame[] = [];
      const run = newRun(async (label, n) => {
        await page.waitForTimeout(MOMENT_DELAY_MS);
        const file = path.join(frameDir, `${String(n).padStart(3, "0")}.jpg`);
        await page.screenshot({ path: file, type: "jpeg", quality: 80 });
        frames.push({ n, label, page: pathOf(page), theme: await shownTheme(page), file: rel(file) });
      });
      const segments: Segment[] = [];
      let completed = true;
      const trace = out("traces", `${session}.zip`);
      try {
        const first = taskById.get(card.tasks[0])!;
        await start(page, first.start, run);
        for (const id of card.tasks) {
          const task = taskById.get(id) as Task;
          if (pathOf(page) !== task.start) {
            await segment(run, segments, "between", "home", () => follow(page, card.between as Step[], card.mode, run));
          }
          for (const twist of card.twists.filter((twist) => twist.before === id)) {
            await segment(run, segments, "twist", twist.id, () => follow(page, twist.route as Step[], card.mode, run));
          }
          await segment(run, segments, "task", id, () => play(page, task, card.mode, { run, fromHere: true }));
        }
      } catch {
        completed = false;
      } finally {
        await context.tracing.stop({ path: trace });
        await context.close();
      }
      const record = {
        session,
        card: card.id,
        kind: card.kind,
        mode: card.mode,
        viewport: viewport.name,
        group,
        completed,
        tasks_done: segments.filter((s) => s.kind === "task" && s.completed).length,
        tasks: card.tasks.length,
        actions: run.actions,
        looks: run.looks,
        steers: run.steers,
        time_to_act_max: Math.max(0, ...run.timeToAct),
        glance_median: nearestRank(run.glances, 0.5),
        glance_p95: nearestRank(run.glances, 0.95),
        step_actions_max: Math.max(0, ...run.steps.map((step) => step.actions)),
        external_requests: refused.length,
        trace: rel(trace),
        log: run.log,
        segments,
        steps: run.steps,
        frames,
      };
      fs.mkdirSync(out("sessions"), { recursive: true });
      fs.writeFileSync(out("sessions", `${session}.json`), JSON.stringify(record, null, 2) + "\n");
    });
  }
}

// The key pages of every task of the round's cards, in every theme: the
// landing page, then each task's truth page reached by the pointer user,
// each taken 0.75 s after it was reached, composed into one sheet per
// viewport with a row per theme.
const sheetTasks = [...new Set(cards.flatMap((card) => card.tasks))];

for (const { viewport } of viewports) {
  test(`contact sheet ${viewport.name}`, async ({ browser }) => {
    type Tile = { theme: string; task: string; page: string; file: string | null; error?: string };
    const tiles: Tile[] = [];
    const dir = out("contact", viewport.name);
    fs.mkdirSync(dir, { recursive: true });
    for (const theme of themes) {
      const refused: string[] = [];
      const context = await browser.newContext({
        viewport: { width: viewport.width, height: viewport.height },
        reducedMotion: "reduce",
      });
      await hermetic(context, refused);
      await chooseTheme(context, theme);
      const page = await context.newPage();
      const shoot = async (task: string) => {
        await page.waitForTimeout(MOMENT_DELAY_MS);
        const file = path.join(dir, `${theme.name}-${task}.jpg`);
        await page.screenshot({ path: file, type: "jpeg", quality: 80 });
        tiles.push({ theme: theme.name, task, page: pathOf(page), file: rel(file) });
      };
      try {
        await start(page, "index.html", newRun());
        await shoot("landing");
        for (const id of sheetTasks) {
          try {
            await play(page, taskById.get(id) as Task, "pointer" as Mode);
            await shoot(id);
          } catch (error) {
            const message = error instanceof Error ? error.message.split("\n")[0] : String(error);
            tiles.push({ theme: theme.name, task: id, page: pathOf(page), file: null, error: message });
          }
        }
      } finally {
        await context.close();
      }
    }
    const sheet = out("sheets", `contact-${viewport.name}.jpg`);
    fs.mkdirSync(path.dirname(sheet), { recursive: true });
    const cell = (tile: Tile) => {
      const image = tile.file
        ? `<img src="data:image/jpeg;base64,${fs.readFileSync(path.join(REPO, tile.file)).toString("base64")}">`
        : `<div class="missing">not reached: ${escapeHtml(tile.error ?? "")}</div>`;
      return `<figure>${image}<figcaption>${escapeHtml(tile.task)}<br>${escapeHtml(tile.page)}</figcaption></figure>`;
    };
    const rows = themes
      .map((theme) => `<div class="row"><h2>${theme.name}</h2>${tiles.filter((t) => t.theme === theme.name).map(cell).join("")}</div>`)
      .join("");
    const html = `<!doctype html><html><head><style>
      body { margin: 0; padding: 12px; background: #fff; font: 13px/1.3 sans-serif; color: #111; }
      h1 { font-size: 16px; margin: 0 0 8px; }
      .row { display: flex; gap: 12px; align-items: flex-start; margin-bottom: 12px; }
      h2 { writing-mode: vertical-rl; font-size: 14px; margin: 0; }
      figure { margin: 0; width: ${THUMB_WIDTH}px; }
      img { width: ${THUMB_WIDTH}px; border: 1px solid #888; display: block; }
      .missing { width: ${THUMB_WIDTH}px; height: 120px; border: 2px dashed #c00; color: #c00; padding: 4px; box-sizing: border-box; }
      figcaption { margin-top: 4px; word-break: break-all; }
    </style></head><body><h1>${round} key pages at ${viewport.name}, each 0.75 s after it was reached</h1>${rows}</body></html>`;
    const context = await browser.newContext({ viewport: { width: 1200, height: 800 } });
    const page = await context.newPage();
    await page.setContent(html, { waitUntil: "load" });
    await page.screenshot({ path: sheet, fullPage: true, type: "jpeg", quality: 80 });
    await context.close();
    fs.writeFileSync(
      out("contact", `${viewport.name}.json`),
      JSON.stringify({ viewport: viewport.name, sheet: rel(sheet), tiles }, null, 2) + "\n",
    );
  });
}

function escapeHtml(text: string): string {
  return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}
