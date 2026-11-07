import type { BrowserContext, Page } from "@playwright/test";
import type { Theme } from "../adapter";

export function baseUrl(): string {
  const base = process.env.UX_BASE_URL;
  if (!base) {
    throw new Error("UX_BASE_URL is unset: the global setup did not run");
  }
  return base;
}

// Hermetic: every request that leaves the local server is refused, so a
// page that loads a remote resource fails visibly instead of measuring the
// network.
export async function hermetic(context: BrowserContext, refused: string[]) {
  const base = baseUrl();
  await context.route("**/*", (route) => {
    const url = route.request().url();
    if (url.startsWith(base) || url.startsWith("data:") || url.startsWith("blob:")) {
      return route.continue();
    }
    refused.push(url);
    return route.abort("blockedbyclient");
  });
}

// The theme a reader chose, as the site stores it, before any script runs.
export async function chooseTheme(context: BrowserContext, theme: Theme) {
  await context.addInitScript((storage) => {
    try {
      localStorage.setItem("mdbook-theme", storage);
    } catch {
      // storage refused: the site's default applies
    }
  }, theme.storage);
}

// The theme a session starts in: written before the first page's scripts
// run and only while storage holds no choice, so a reader's switch stays
// on the next page.
export async function seedTheme(context: BrowserContext, theme: Theme) {
  await context.addInitScript((storage) => {
    try {
      if (localStorage.getItem("mdbook-theme") === null) {
        localStorage.setItem("mdbook-theme", storage);
      }
    } catch {
      // storage refused: the site's default applies
    }
  }, theme.storage);
}

export async function open(page: Page, path: string) {
  await page.goto(baseUrl() + path, { waitUntil: "load" });
  await page.evaluate(() => document.fonts.ready);
  await settle(page);
}

// Waits until the page stops moving: two frames with the same scroll
// position and no pending layout.
export async function settle(page: Page) {
  await page.evaluate(
    () =>
      new Promise<void>((resolve) => {
        let last = -1;
        let same = 0;
        const tick = () => {
          const now = scrollY * 100000 + scrollX;
          same = now === last ? same + 1 : 0;
          last = now;
          if (same >= 2) {
            resolve();
          } else {
            requestAnimationFrame(tick);
          }
        };
        requestAnimationFrame(tick);
      }),
  );
}
