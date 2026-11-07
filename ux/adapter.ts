// The adapter of the ui-iteration-loop for the public website (story
// docs-site-ui-loop). Everything site-specific the gates and the
// screen-only user know is here; lib/ holds no selector of its own.

export type Viewport = { name: string; width: number; height: number };

export type Theme = {
  name: string;
  storage: string;
};

export type Control = {
  label: string;
  selector: string;
  when?: (width: number) => boolean;
};

export type PageKind = "landing" | "book";

export type Disclosure = {
  role: "button" | "link";
  name: string;
  open: string;
};

export const SITE_DIR = "target/site";
export const OUT_DIR = "target/ux";

export const gatingViewports: Viewport[] = [
  { name: "1440x900", width: 1440, height: 900 },
  { name: "390x844", width: 390, height: 844 },
];

export const measuredViewports: Viewport[] = [
  { name: "1280x800", width: 1280, height: 800 },
  { name: "1024x768", width: 1024, height: 768 },
  { name: "768x1024", width: 768, height: 1024 },
];

// The themes the site ships: the book's theme menu offers Light and Dark
// (`coal`), Auto resolves to one of them (site/book/theme/index.hbs); the
// landing page reads the same `mdbook-theme` key and has a dark and a light
// theme (site/index.html).
export const themes: Theme[] = [
  { name: "light", storage: "light" },
  { name: "dark", storage: "coal" },
];

const LANDING_NAV_WIDTH = 721;
const BOOK_SIDEBAR_WIDTH = 1080;

export const alwaysVisible: Record<PageKind, Control[]> = {
  landing: [
    { label: "home link", selector: "header.top .brand" },
    { label: "theme toggle", selector: "header.top .theme-toggle" },
    { label: "main navigation", selector: "#main-nav", when: (w) => w >= LANDING_NAV_WIDTH },
    { label: "menu toggle", selector: "header.top .menu-toggle", when: (w) => w < LANDING_NAV_WIDTH },
  ],
  book: [
    { label: "menu bar", selector: "#mdbook-menu-bar" },
    { label: "home link", selector: "#mdbook-menu-bar .ignite-brand" },
    { label: "sidebar toggle", selector: "#mdbook-sidebar-toggle" },
    { label: "theme toggle", selector: "#mdbook-theme-toggle" },
    { label: "search toggle", selector: "#mdbook-search-toggle" },
    { label: "sidebar", selector: "#mdbook-sidebar", when: (w) => w >= BOOK_SIDEBAR_WIDTH },
  ],
};

// Where a control counts as glance 1: the navigation chrome a reader looks
// at first. A control elsewhere in view is glance 2, one that needed a
// reveal glance 3.
export const primaryRegions: Record<PageKind, string> = {
  landing: "header.top",
  book: "#mdbook-menu-bar, #mdbook-sidebar, #mdbook-search-wrapper",
};

// Controls that reveal navigation that is not rendered: the screen-only
// user opens one when no matching control is rendered at all.
export const disclosures: Record<PageKind, Disclosure[]> = {
  landing: [{ role: "button", name: "Menu", open: "header.top.open" }],
  book: [{ role: "button", name: "Toggle Table of Contents", open: "html.sidebar-visible" }],
};

export function pageKind(path: string): PageKind {
  return path.startsWith("book/") ? "book" : "landing";
}

// No guidance cue: no L0 and no gate 6 (SKILL.md section 1).
export const guidanceCue = null;
