import * as fs from "node:fs";
import * as path from "node:path";
import type { Page } from "@playwright/test";
import { OUT_DIR } from "../adapter";

export const UX_DIR = path.resolve(__dirname, "..");
export const REPO = path.resolve(UX_DIR, "..");

type PageList = { root: string; exclude: string[] };

// A stub mdBook writes for `[output.html.redirect]`: it navigates away
// as it loads, so it has no layout of its own to measure.
function redirect(file: string): boolean {
  return fs.readFileSync(file, "utf8").includes('<meta http-equiv="refresh"');
}

function walk(dir: string, base: string, out: string[]) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      walk(full, base, out);
    } else if (entry.name.endsWith(".html") && !redirect(full)) {
      out.push(path.relative(base, full).split(path.sep).join("/"));
    }
  }
}

// The pages of the built site, in byte order, as paths relative to its root.
export function sitePages(): string[] {
  const list: PageList = JSON.parse(
    fs.readFileSync(path.join(UX_DIR, "scenarios/pages/pages.json"), "utf8"),
  );
  const root = path.join(REPO, list.root);
  if (!fs.existsSync(path.join(root, "index.html"))) {
    throw new Error(`${root}/index.html is missing: build the site first (task site)`);
  }
  const out: string[] = [];
  walk(root, root, out);
  return out.filter((page) => !list.exclude.includes(page)).sort();
}

export type Step =
  | { click: { role: "link" | "button"; name: string } }
  | { type: string };

export type Task = {
  id: string;
  goal: string;
  start: string;
  truth: { path: string; text: string };
  route: Step[];
};

// The tasks under ux/scenarios/<id>/task.json, in byte order of their ids.
export function tasks(): Task[] {
  const dir = path.join(UX_DIR, "scenarios");
  return fs
    .readdirSync(dir, { withFileTypes: true })
    .filter((entry) => entry.isDirectory() && fs.existsSync(path.join(dir, entry.name, "task.json")))
    .map((entry) => entry.name)
    .sort()
    .map((id) => {
      const task = JSON.parse(fs.readFileSync(path.join(dir, id, "task.json"), "utf8")) as Task;
      if (task.id !== id) {
        throw new Error(`ux/scenarios/${id}/task.json names the id ${task.id}`);
      }
      return task;
    });
}

// The screen of a page that missed a gating bar, for the round's report:
// target/ux/missed/<viewport>/<bar>/<name>.jpg.
export async function missedShot(page: Page, viewport: string, bar: string, name: string) {
  const dir = path.join(REPO, OUT_DIR, "missed", viewport, bar);
  fs.mkdirSync(dir, { recursive: true });
  await page.screenshot({ path: path.join(dir, `${name.replace(/[^A-Za-z0-9._-]/g, "_")}.jpg`), type: "jpeg", quality: 80 });
}

// One raw record per test, gathered into target/ux/metrics.json by the
// global teardown.
export function writeRecord(outDir: string, name: string, record: unknown) {
  const dir = path.join(REPO, outDir, "raw");
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(path.join(dir, `${name.replace(/[^A-Za-z0-9._-]/g, "_")}.json`), JSON.stringify(record));
}
