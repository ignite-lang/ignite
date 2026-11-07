import type { Locator, Page } from "@playwright/test";
import { disclosures, pageKind, primaryRegions } from "../adapter";
import { baseUrl, settle } from "./browse";
import type { Step, Task } from "./pages";

// The screen-only user of SKILL.md section 7: deterministic, no model. It
// acts only on controls found by accessible role and name that are fully
// inside the viewport and uncovered at their centre. A click, a wheel turn
// or a key press is one action; each inspection of the screen is one look;
// an action that only reveals (a scroll, opening a menu) is a steer.

const MAX_LOOKS = 40;
const MAX_TABS = 400;

export type Run = {
  actions: number;
  looks: number;
  steers: number;
  timeToAct: number[];
  glances: number[];
  log: string[];
};

type Seen = { index: number; x: number; y: number; primary: boolean; href: string | null };

function currentPath(page: Page): string {
  const url = new URL(page.url());
  return url.pathname.replace(/^\//, "");
}

function candidates(page: Page, step: { role: "link" | "button"; name: string }): Locator {
  return page.getByRole(step.role, { name: new RegExp(step.name) });
}

// Runs `action`; when it follows a link to another page, waits for that
// page to load first, so no look reads a page that is going away.
async function act(page: Page, href: string | null, action: () => Promise<void>) {
  const here = new URL(page.url());
  const there = href ? new URL(href) : null;
  if (there && there.origin === here.origin && there.pathname !== here.pathname) {
    await Promise.all([page.waitForURL((url) => url.pathname === there.pathname, { waitUntil: "load" }), action()]);
  } else {
    await action();
  }
  await quiet(page);
}

// After an action: the load finished, scrolling stopped and the DOM was
// quiet for 150 ms.
async function quiet(page: Page) {
  await page.waitForLoadState("load");
  await settle(page);
  await page.evaluate(
    () =>
      new Promise<void>((resolve) => {
        let timer = setTimeout(done, 150);
        const observer = new MutationObserver(() => {
          clearTimeout(timer);
          timer = setTimeout(done, 150);
        });
        function done() {
          observer.disconnect();
          resolve();
        }
        observer.observe(document.documentElement, { subtree: true, childList: true, attributes: true, characterData: true });
      }),
  );
}

// The first candidate usable by the screen-only user, or null.
async function usable(page: Page, all: Locator, primary: string): Promise<Seen | null> {
  const count = await all.count();
  for (let index = 0; index < count; index++) {
    const seen = await all.nth(index).evaluate(
      (el, primary) => {
        const style = getComputedStyle(el);
        const rect = el.getBoundingClientRect();
        if (style.visibility === "hidden" || rect.width === 0 || rect.height === 0) {
          return null;
        }
        // A control nobody can see (opacity 0 on it or above it) is not used.
        if (!el.checkVisibility({ opacityProperty: true, visibilityProperty: true })) {
          return null;
        }
        if (rect.left < 0 || rect.top < 0 || rect.right > innerWidth || rect.bottom > innerHeight) {
          return null;
        }
        // A link that wraps has one box per line: its centre is the centre
        // of its largest line box, where a reader would click.
        const line = Array.from(el.getClientRects()).reduce((a, b) => (b.width * b.height > a.width * a.height ? b : a));
        const x = line.left + line.width / 2;
        const y = line.top + line.height / 2;
        const hit = document.elementFromPoint(x, y);
        if (!hit || !(hit === el || el.contains(hit))) {
          return null;
        }
        const link = el.closest("a");
        return { x, y, primary: el.closest(primary) !== null, href: link ? link.href : null };
      },
      primary,
    );
    if (seen) {
      return { index, ...seen };
    }
  }
  return null;
}

// One wheel turn toward the first rendered candidate, in the box that
// scrolls it; false when no candidate is rendered.
async function scrollToward(page: Page, all: Locator, run: Run): Promise<boolean> {
  const count = await all.count();
  for (let index = 0; index < count; index++) {
    const plan = await all.nth(index).evaluate((el) => {
      const rect = el.getBoundingClientRect();
      if (getComputedStyle(el).visibility === "hidden" || rect.width === 0 || rect.height === 0) {
        return null;
      }
      // Beside the viewport (a closed drawer): no vertical scroll reaches it.
      if (rect.right <= 0 || rect.left >= innerWidth) {
        return null;
      }
      let box: Element | null = el.parentElement;
      while (box && box !== document.body && box !== document.documentElement) {
        const style = getComputedStyle(box);
        if (/(auto|scroll)/.test(style.overflowY) && box.scrollHeight > box.clientHeight) {
          break;
        }
        box = box.parentElement;
      }
      const page = !box || box === document.body || box === document.documentElement;
      const frame = page
        ? { left: 0, top: 0, right: innerWidth, bottom: innerHeight }
        : box!.getBoundingClientRect();
      const top = Math.max(frame.top, 0);
      const bottom = Math.min(frame.bottom, innerHeight);
      const height = bottom - top;
      let dy = 0;
      if (rect.bottom > bottom) {
        dy = Math.round(height * 0.8);
      } else if (rect.top < top) {
        dy = -Math.round(height * 0.8);
      } else {
        dy = -Math.round(height / 3);
      }
      return {
        x: Math.round((Math.max(frame.left, 0) + Math.min(frame.right, innerWidth)) / 2),
        y: Math.round((top + bottom) / 2),
        dy,
      };
    });
    if (plan) {
      await page.mouse.move(plan.x, plan.y);
      await page.mouse.wheel(0, plan.dy);
      run.actions++;
      run.steers++;
      run.log.push(`scroll ${plan.dy > 0 ? "down" : "up"}`);
      await quiet(page);
      return true;
    }
  }
  return false;
}

// Opens the first closed disclosure that is usable; false when none is.
async function disclose(page: Page, run: Run): Promise<boolean> {
  const kind = pageKind(currentPath(page));
  for (const disclosure of disclosures[kind]) {
    const open = await page.evaluate((selector) => document.querySelector(selector) !== null, disclosure.open);
    if (open) {
      continue;
    }
    const seen = await usable(
      page,
      page.getByRole(disclosure.role, { name: disclosure.name, exact: true }),
      primaryRegions[kind],
    );
    if (seen) {
      await page.mouse.click(seen.x, seen.y);
      run.actions++;
      run.steers++;
      run.log.push(`open ${disclosure.name}`);
      await quiet(page);
      return true;
    }
  }
  return false;
}

async function pointerClick(page: Page, step: { role: "link" | "button"; name: string }, run: Run) {
  const all = candidates(page, step);
  let looks = 0;
  let revealed = false;
  while (looks < MAX_LOOKS) {
    looks++;
    run.looks++;
    const kind = pageKind(currentPath(page));
    const seen = await usable(page, all, primaryRegions[kind]);
    if (seen) {
      run.glances.push(revealed ? 3 : seen.primary ? 1 : 2);
      run.timeToAct.push(looks);
      run.actions++;
      run.log.push(`click ${step.role} /${step.name}/`);
      await act(page, seen.href, () => page.mouse.click(seen.x, seen.y));
      return;
    }
    revealed = true;
    if (await scrollToward(page, all, run)) {
      continue;
    }
    if (await disclose(page, run)) {
      continue;
    }
    break;
  }
  throw new Error(`no usable ${step.role} named /${step.name}/ on ${currentPath(page)} after ${looks} looks`);
}

async function typeText(page: Page, text: string, run: Run) {
  const editable = await page.evaluate(() => {
    const el = document.activeElement;
    return el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement;
  });
  if (!editable) {
    throw new Error(`nothing to type "${text}" into on ${currentPath(page)}: the focus is not on a text field`);
  }
  for (const key of text) {
    await page.keyboard.press(key);
    run.actions++;
  }
  run.log.push(`type ${text}`);
  await quiet(page);
}

async function focused(all: Locator): Promise<boolean> {
  const count = await all.count();
  for (let index = 0; index < count; index++) {
    const seen = await all
      .nth(index)
      .evaluate(
        (el) => el === document.activeElement && el.checkVisibility({ opacityProperty: true, visibilityProperty: true }),
      );
    if (seen) {
      return true;
    }
  }
  return false;
}

async function rendered(all: Locator): Promise<boolean> {
  const count = await all.count();
  for (let index = 0; index < count; index++) {
    if (await all.nth(index).isVisible()) {
      return true;
    }
  }
  return false;
}

async function keyboardClick(page: Page, step: { role: "link" | "button"; name: string }, run: Run) {
  const all = candidates(page, step);
  for (let tabs = 0; tabs <= MAX_TABS; tabs++) {
    if (await focused(all)) {
      const href = await page.evaluate(() => {
        const link = document.activeElement?.closest("a");
        return link ? link.href : null;
      });
      run.actions++;
      run.log.push(`enter on ${step.role} /${step.name}/ after ${tabs} tabs`);
      await act(page, href, () => page.keyboard.press("Enter"));
      return;
    }
    if (!(await rendered(all))) {
      const kind = pageKind(currentPath(page));
      for (const disclosure of disclosures[kind]) {
        const open = await page.evaluate((selector) => document.querySelector(selector) !== null, disclosure.open);
        if (!open && (await focused(page.getByRole(disclosure.role, { name: disclosure.name, exact: true })))) {
          await page.keyboard.press("Enter");
          run.actions++;
          run.steers++;
          run.log.push(`open ${disclosure.name}`);
          await quiet(page);
        }
      }
    }
    await page.keyboard.press("Tab");
    run.actions++;
  }
  throw new Error(`no ${step.role} named /${step.name}/ reached by Tab on ${currentPath(page)} in ${MAX_TABS} presses`);
}

export type Mode = "pointer" | "keyboard";

export async function play(page: Page, task: Task, mode: Mode): Promise<Run> {
  const run: Run = { actions: 0, looks: 0, steers: 0, timeToAct: [], glances: [], log: [] };
  await page.goto(baseUrl() + task.start, { waitUntil: "load" });
  await quiet(page);
  for (const step of task.route as Step[]) {
    if ("type" in step) {
      await typeText(page, step.type, run);
    } else if (mode === "pointer") {
      await pointerClick(page, step.click, run);
    } else {
      await keyboardClick(page, step.click, run);
    }
  }
  const at = currentPath(page);
  if (at !== task.truth.path) {
    throw new Error(`${task.id} (${mode}) ended on ${at}, not on its truth page ${task.truth.path}; ${run.log.join(", ")}`);
  }
  const text = await page.evaluate(() => document.body.innerText);
  if (!text.includes(task.truth.text)) {
    throw new Error(`${task.id} (${mode}) reached ${at} but the page does not show "${task.truth.text}"`);
  }
  return run;
}

export function nearestRank(values: number[], fraction: number): number {
  if (!values.length) {
    return 0;
  }
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.max(0, Math.ceil(fraction * sorted.length) - 1)];
}
