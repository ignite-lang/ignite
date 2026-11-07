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

// One step of a route as the user took it: the moment (frame) at which it
// began to look, the frame on which it found the control (the last screen
// before the click; for the keyboard the screen with the focus on it,
// before Enter), how many looks (pointer) or Tab presses (keyboard) it
// took, its glance cost (pointer only, null for the keyboard) and its
// actions.
export type StepRecord = {
  label: string;
  page: string;
  frame: number;
  found: number;
  looks: number;
  glance: number | null;
  actions: number;
};

export type Run = {
  actions: number;
  looks: number;
  steers: number;
  timeToAct: number[];
  glances: number[];
  log: string[];
  // Moments: one after the start and after every action that changes the
  // screen (a click, a wheel turn, an opened menu, typed text, an Enter).
  // A Tab press only moves the focus and is no moment.
  moments: number;
  steps: StepRecord[];
  observer?: (label: string, frame: number) => Promise<void>;
};

export function newRun(observer?: Run["observer"]): Run {
  return { actions: 0, looks: 0, steers: 0, timeToAct: [], glances: [], log: [], moments: 0, steps: [], observer };
}

async function moment(run: Run, label: string) {
  run.moments++;
  if (run.observer) {
    await run.observer(label, run.moments);
  }
}

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
async function usable(page: Page, all: Locator, primary: string, focusedOnly = false): Promise<Seen | null> {
  const count = await all.count();
  for (let index = 0; index < count; index++) {
    const seen = await all.nth(index).evaluate(
      (el, { primary, focusedOnly }) => {
        if (focusedOnly && el !== document.activeElement) {
          return null;
        }
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
      { primary, focusedOnly },
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
      await moment(run, run.log[run.log.length - 1]);
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
      await moment(run, run.log[run.log.length - 1]);
      return true;
    }
  }
  return false;
}

async function pointerClick(page: Page, step: { role: "link" | "button"; name: string }, run: Run) {
  const all = candidates(page, step);
  const frame = run.moments;
  const before = run.actions;
  let looks = 0;
  let revealed = false;
  while (looks < MAX_LOOKS) {
    looks++;
    run.looks++;
    const kind = pageKind(currentPath(page));
    const seen = await usable(page, all, primaryRegions[kind]);
    if (seen) {
      const page0 = currentPath(page);
      const found = run.moments;
      run.glances.push(revealed ? 3 : seen.primary ? 1 : 2);
      run.timeToAct.push(looks);
      run.actions++;
      run.log.push(`click ${step.role} /${step.name}/`);
      await act(page, seen.href, () => page.mouse.click(seen.x, seen.y));
      await moment(run, run.log[run.log.length - 1]);
      run.steps.push({ label: run.log[run.log.length - 1], page: page0, frame, found, looks, glance: revealed ? 3 : seen.primary ? 1 : 2, actions: run.actions - before });
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
  await moment(run, run.log[run.log.length - 1]);
}

// Whether one of `all` has the focus and is usable as the pointer user
// requires: visible, opaque, fully in the viewport and uncovered. A
// keyboard user who cannot see the focused control does not press Enter.
async function focused(page: Page, all: Locator): Promise<boolean> {
  const kind = pageKind(currentPath(page));
  return (await usable(page, all, primaryRegions[kind], true)) !== null;
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
  const frame = run.moments;
  const before = run.actions;
  for (let tabs = 0; tabs <= MAX_TABS; tabs++) {
    if (await focused(page, all)) {
      const href = await page.evaluate(() => {
        const link = document.activeElement?.closest("a");
        return link ? link.href : null;
      });
      const page0 = currentPath(page);
      // For the keyboard user a look is a Tab press and the check of
      // where the focus went.
      run.looks += tabs;
      run.timeToAct.push(tabs);
      // The screen with the focus on the control, before Enter: a moment
      // of its own, since a Tab press is none.
      await moment(run, `focus on ${step.role} /${step.name}/ after ${tabs} tabs`);
      const found = run.moments;
      run.actions++;
      run.log.push(`enter on ${step.role} /${step.name}/ after ${tabs} tabs`);
      await act(page, href, () => page.keyboard.press("Enter"));
      await moment(run, run.log[run.log.length - 1]);
      run.steps.push({ label: run.log[run.log.length - 1], page: page0, frame, found, looks: tabs, glance: null, actions: run.actions - before });
      return;
    }
    if (!(await rendered(all))) {
      const kind = pageKind(currentPath(page));
      for (const disclosure of disclosures[kind]) {
        const open = await page.evaluate((selector) => document.querySelector(selector) !== null, disclosure.open);
        if (!open && (await focused(page, page.getByRole(disclosure.role, { name: disclosure.name, exact: true })))) {
          await page.keyboard.press("Enter");
          run.actions++;
          run.steers++;
          run.log.push(`open ${disclosure.name}`);
          await quiet(page);
          await moment(run, run.log[run.log.length - 1]);
        }
      }
    }
    await page.keyboard.press("Tab");
    run.actions++;
  }
  throw new Error(`no ${step.role} named /${step.name}/ reached by Tab on ${currentPath(page)} in ${MAX_TABS} presses`);
}

export type Mode = "pointer" | "keyboard";

// Takes `route` from where the page is, with the pointer or the keyboard.
export async function follow(page: Page, route: Step[], mode: Mode, run: Run) {
  for (const step of route) {
    if ("type" in step) {
      await typeText(page, step.type, run);
    } else if (mode === "pointer") {
      await pointerClick(page, step.click, run);
    } else {
      await keyboardClick(page, step.click, run);
    }
  }
}

// Opens `path` by address: the start of a task or a session, no action.
export async function start(page: Page, path: string, run: Run) {
  await page.goto(baseUrl() + path, { waitUntil: "load" });
  await quiet(page);
  await moment(run, `open ${path}`);
}

// Plays `task`. With `fromHere` the user is already on the task's start
// page (a session that walked there) and nothing is opened by address.
export async function play(
  page: Page,
  task: Task,
  mode: Mode,
  options: { run?: Run; fromHere?: boolean } = {},
): Promise<Run> {
  const run = options.run ?? newRun();
  if (!options.fromHere) {
    await start(page, task.start, run);
  }
  await follow(page, task.route as Step[], mode, run);
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
