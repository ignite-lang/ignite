import type { Page } from "@playwright/test";
import type { Control } from "../adapter";

export type Problem = { check: string; detail: string };

const TOLERANCE = 1;

// Hard gate 1 for a docs site: no horizontal page scroll, no element whose
// content runs past its box. A scroll container (overflow auto or scroll)
// holds its content, so it is no overflow; text cut by a clipping box is
// the truncation check's business.
export async function overflow(page: Page): Promise<Problem[]> {
  return page.evaluate((tolerance) => {
    const problems: { check: string; detail: string }[] = [];
    const root = document.documentElement;
    if (root.scrollWidth > root.clientWidth + tolerance) {
      problems.push({
        check: "horizontal page scroll",
        detail: `document ${root.scrollWidth}px wide in a ${root.clientWidth}px viewport`,
      });
    }
    const describe = (el: Element) => {
      const id = el.id ? `#${el.id}` : "";
      const cls = typeof el.className === "string" && el.className.trim() ? `.${el.className.trim().split(/\s+/).join(".")}` : "";
      const text = (el.textContent ?? "").trim().replace(/\s+/g, " ").slice(0, 40);
      return `${el.tagName.toLowerCase()}${id}${cls} "${text}"`;
    };
    const rendered = (el: Element) => {
      const style = getComputedStyle(el);
      return style.display !== "none" && style.visibility !== "hidden" && el.getClientRects().length > 0;
    };
    const scrolls = (el: Element) => /(auto|scroll)/.test(getComputedStyle(el).overflowX);
    const clips = (el: Element) => /(hidden|clip)/.test(getComputedStyle(el).overflowX);
    for (const el of Array.from(document.body.querySelectorAll("*"))) {
      if (!(el instanceof HTMLElement) || !rendered(el)) {
        continue;
      }
      const style = getComputedStyle(el);
      if (style.display === "inline" || el.clientWidth === 0) {
        continue;
      }
      if (scrolls(el) || clips(el)) {
        continue;
      }
      if (el.scrollWidth > el.clientWidth + tolerance) {
        let inside = false;
        for (let up = el.parentElement; up; up = up.parentElement) {
          if (scrolls(up) || clips(up)) {
            inside = true;
            break;
          }
        }
        if (!inside) {
          problems.push({
            check: "element past its box",
            detail: `${describe(el)}: content ${el.scrollWidth}px in a ${el.clientWidth}px box`,
          });
        }
      }
    }
    return problems;
  }, TOLERANCE);
}

// Text cut off by its box (an ellipsis, a clipping box or a line clamp) must
// carry its full value in `title` or `aria-label`.
export async function truncation(page: Page): Promise<Problem[]> {
  return page.evaluate((tolerance) => {
    const problems: { check: string; detail: string }[] = [];
    for (const el of Array.from(document.body.querySelectorAll("*"))) {
      if (!(el instanceof HTMLElement)) {
        continue;
      }
      const style = getComputedStyle(el);
      if (style.display === "none" || style.visibility === "hidden" || el.getClientRects().length === 0) {
        continue;
      }
      const clipsX = /(hidden|clip)/.test(style.overflowX);
      const clipsY = /(hidden|clip)/.test(style.overflowY);
      const clamped = style.webkitLineClamp !== "none" && style.webkitLineClamp !== "";
      const cutX = clipsX && el.scrollWidth > el.clientWidth + tolerance;
      const cutY = (clipsY || clamped) && el.scrollHeight > el.clientHeight + tolerance;
      if (!cutX && !cutY) {
        continue;
      }
      const text = (el.innerText ?? "").trim();
      if (!text) {
        continue;
      }
      const full = `${el.getAttribute("title") ?? ""} ${el.getAttribute("aria-label") ?? ""}`;
      if (full.replace(/\s+/g, " ").includes(text.replace(/\s+/g, " "))) {
        continue;
      }
      // Cut text: a line of text this box clips, not one held by a nearer
      // clipping or scrolling box (that box answers for its own text).
      const box = el.getBoundingClientRect();
      const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
      let cut = false;
      for (let node = walker.nextNode(); node && !cut; node = walker.nextNode()) {
        if (!node.textContent?.trim()) {
          continue;
        }
        let nearer = false;
        for (let up = node.parentElement; up && up !== el; up = up.parentElement) {
          const s = getComputedStyle(up);
          if (s.overflowX !== "visible" || s.overflowY !== "visible") {
            nearer = true;
            break;
          }
        }
        if (nearer) {
          continue;
        }
        const range = document.createRange();
        range.selectNodeContents(node);
        cut = Array.from(range.getClientRects()).some(
          (rect) =>
            rect.width > 0 &&
            (rect.right > box.right + tolerance ||
              rect.left < box.left - tolerance ||
              rect.bottom > box.bottom + tolerance ||
              rect.top < box.top - tolerance),
        );
      }
      if (!cut) {
        continue;
      }
      problems.push({
        check: "truncated text",
        detail: `${el.tagName.toLowerCase()}${el.id ? "#" + el.id : ""} "${text.replace(/\s+/g, " ").slice(0, 60)}"`,
      });
    }
    return problems;
  }, TOLERANCE);
}

// Hard gate 2: each always-visible control is rendered, 100 % inside the
// viewport, and the element at its centre is the control or inside it.
export async function alwaysVisible(page: Page, controls: Control[], width: number): Promise<Problem[]> {
  const wanted = controls.filter((control) => !control.when || control.when(width));
  return page.evaluate(
    ({ wanted, tolerance }) => {
      const problems: { check: string; detail: string }[] = [];
      for (const control of wanted) {
        const el = document.querySelector(control.selector);
        if (!el) {
          problems.push({ check: "always visible", detail: `${control.label} (${control.selector}) is missing` });
          continue;
        }
        const style = getComputedStyle(el);
        const rect = el.getBoundingClientRect();
        if (style.display === "none" || style.visibility === "hidden" || rect.width === 0 || rect.height === 0) {
          problems.push({ check: "always visible", detail: `${control.label} is not rendered` });
          continue;
        }
        if (
          rect.left < -tolerance ||
          rect.top < -tolerance ||
          rect.right > innerWidth + tolerance ||
          rect.bottom > innerHeight + tolerance
        ) {
          problems.push({
            check: "always visible",
            detail: `${control.label} is not fully in view: ${Math.round(rect.left)},${Math.round(rect.top)} to ${Math.round(rect.right)},${Math.round(rect.bottom)} in ${innerWidth}x${innerHeight}`,
          });
          continue;
        }
        const x = rect.left + rect.width / 2;
        const y = rect.top + rect.height / 2;
        const hit = document.elementFromPoint(x, y);
        if (!hit || !(hit === el || el.contains(hit))) {
          problems.push({
            check: "always visible",
            detail: `${control.label} is covered at its centre by ${hit ? hit.tagName.toLowerCase() + (hit.id ? "#" + hit.id : "") : "nothing"}`,
          });
        }
      }
      return problems;
    },
    { wanted: wanted.map(({ label, selector }) => ({ label, selector })), tolerance: TOLERANCE },
  );
}

// Reduced motion honoured: with `prefers-reduced-motion: reduce` no
// animation or transition runs on the page.
export async function motion(page: Page): Promise<Problem[]> {
  return page.evaluate(() =>
    document
      .getAnimations()
      .filter((animation) => animation.playState === "running")
      .map((animation) => {
        const target = (animation.effect as KeyframeEffect | null)?.target;
        const name = (animation as CSSAnimation).animationName ?? (animation as CSSTransition).transitionProperty ?? "script";
        return {
          check: "reduced motion",
          detail: `${name} runs on ${target ? target.tagName.toLowerCase() + (target.id ? "#" + target.id : "") : "?"}`,
        };
      }),
  );
}
