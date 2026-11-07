import type { Page } from "@playwright/test";
import { type Control, scrollRegions } from "../adapter";

export type Problem = { check: string; detail: string };

const TOLERANCE = 1;

// Hard gate 1 for a docs site: no horizontal page scroll and no element
// whose content runs past its box.
//
// - The document scrolls sideways: a horizontal page scroll.
// - A box that scrolls sideways and is not one of the adapter's scroll
//   regions (a code block, a table, ...): a horizontal page scroll too. On a
//   book page the reading column, #mdbook-content, scrolls, so a wide block
//   there scrolls the page as a reader sees it.
// - A box that clips (overflow hidden or clip) and cuts off text or a
//   control sideways: content past its box. Decoration (no text, no
//   control, aria-hidden) may be clipped.
// - A box that neither scrolls nor clips and whose content runs past it,
//   with no scrolling or clipping box above to answer for it: content past
//   its box.
export async function overflow(page: Page, regions: string = scrollRegions): Promise<Problem[]> {
  return page.evaluate(
    ({ tolerance, regions }) => {
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
        const cls =
          typeof el.className === "string" && el.className.trim() ? `.${el.className.trim().split(/\s+/).join(".")}` : "";
        const text = (el.textContent ?? "").trim().replace(/\s+/g, " ").slice(0, 40);
        return `${el.tagName.toLowerCase()}${id}${cls} "${text}"`;
      };
      const rendered = (el: Element) => {
        const style = getComputedStyle(el);
        return style.display !== "none" && style.visibility !== "hidden" && el.getClientRects().length > 0;
      };
      const scrolls = (el: Element) => /(auto|scroll)/.test(getComputedStyle(el).overflowX);
      const clips = (el: Element) => /(hidden|clip)/.test(getComputedStyle(el).overflowX);
      const boxed = (el: Element) => scrolls(el) || clips(el);
      const nearestBox = (el: Element) => {
        for (let up = el.parentElement; up; up = up.parentElement) {
          if (boxed(up)) {
            return up;
          }
        }
        return null;
      };
      const meaningful = (el: Element) => {
        if (el.closest("[aria-hidden=true]")) {
          return false;
        }
        if (el.matches("a, button, input, select, textarea, img, [tabindex]")) {
          return true;
        }
        return Array.from(el.childNodes).some((n) => n.nodeType === Node.TEXT_NODE && (n.textContent ?? "").trim() !== "");
      };
      for (const el of Array.from(document.body.querySelectorAll("*"))) {
        if (!(el instanceof HTMLElement) || !rendered(el)) {
          continue;
        }
        if (getComputedStyle(el).display === "inline" || el.clientWidth === 0) {
          continue;
        }
        if (el.scrollWidth <= el.clientWidth + tolerance) {
          continue;
        }
        if (scrolls(el)) {
          if (!el.matches(regions)) {
            problems.push({
              check: "horizontal page scroll",
              detail: `${describe(el)} scrolls sideways: content ${el.scrollWidth}px in a ${el.clientWidth}px box`,
            });
          }
          continue;
        }
        if (clips(el)) {
          // Cut text whose full value the box carries (an ellipsis with a
          // title) is the truncation check's allowed case, not an overflow.
          const full = `${el.getAttribute("title") ?? ""} ${el.getAttribute("aria-label") ?? ""}`.replace(/\s+/g, " ");
          const own = el.innerText.trim().replace(/\s+/g, " ");
          if (own && full.includes(own)) {
            continue;
          }
          const box = el.getBoundingClientRect();
          const past = (rects: DOMRectList | DOMRect[]) =>
            Array.from(rects).some(
              (rect) => rect.width > 0 && (rect.right > box.right + tolerance || rect.left < box.left - tolerance),
            );
          let cut: Element | undefined = Array.from(el.querySelectorAll("a, button, input, select, textarea, img, [tabindex]")).find(
            (inner) => rendered(inner) && nearestBox(inner) === el && meaningful(inner) && past(inner.getClientRects()),
          );
          const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
          for (let node = walker.nextNode(); node && !cut; node = walker.nextNode()) {
            const parent = node.parentElement;
            if (!parent || !(node.textContent ?? "").trim() || !rendered(parent) || !meaningful(parent)) {
              continue;
            }
            // Text this box answers for: its own, or in a box that neither
            // scrolls nor clips below it.
            if (!(parent === el || (!boxed(parent) && nearestBox(parent) === el))) {
              continue;
            }
            const range = document.createRange();
            range.selectNodeContents(node);
            if (past(range.getClientRects())) {
              cut = parent;
            }
          }
          if (cut) {
            problems.push({
              check: "element past its box",
              detail: `${describe(cut)} is cut off sideways by ${describe(el)}`,
            });
          }
          continue;
        }
        if (nearestBox(el) === null) {
          problems.push({
            check: "element past its box",
            detail: `${describe(el)}: content ${el.scrollWidth}px in a ${el.clientWidth}px box`,
          });
        }
      }
      return problems;
    },
    { tolerance: TOLERANCE, regions },
  );
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

// Hard gate 2: each always-visible control is rendered and opaque, 100 % inside the
// viewport, and uncovered: every sampled point of it hits the control.
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
        // Transparent counts as hidden: opacity 0 on the control or above it.
        if (!el.checkVisibility({ opacityProperty: true, visibilityProperty: true })) {
          problems.push({ check: "always visible", detail: `${control.label} is transparent or hidden` });
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
        // Uncovered as a whole: the centre and a 5 by 5 grid over the
        // control's central 70 % (a round button's corners are not part of
        // it), all hit the control.
        const points: [number, number, string][] = [[rect.left + rect.width / 2, rect.top + rect.height / 2, "its centre"]];
        const insetX = rect.width * 0.15;
        const insetY = rect.height * 0.15;
        for (let i = 0; i < 5; i++) {
          for (let j = 0; j < 5; j++) {
            const x = rect.left + insetX + ((rect.width - 2 * insetX) * i) / 4;
            const y = rect.top + insetY + ((rect.height - 2 * insetY) * j) / 4;
            points.push([x, y, `${Math.round(x)},${Math.round(y)}`]);
          }
        }
        for (const [x, y, where] of points) {
          const hit = document.elementFromPoint(x, y);
          if (!hit || !(hit === el || el.contains(hit))) {
            problems.push({
              check: "always visible",
              detail: `${control.label} is covered at ${where} by ${hit ? hit.tagName.toLowerCase() + (hit.id ? "#" + hit.id : "") : "nothing"}`,
            });
            break;
          }
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
