import type { Browser, BrowserContext } from "@playwright/test";
import { themes, type Viewport } from "../adapter";
import { hermetic, seedTheme } from "./browse";
import type { Card } from "./cards";

// The browser context of one trial session: the card's viewport, hermetic,
// reduced motion (so the counts repeat), and the theme the card starts in,
// set once: a theme the session switches to stays on later pages.
export async function sessionContext(
  browser: Browser,
  card: Card,
  viewport: Viewport,
  refused: string[],
): Promise<BrowserContext> {
  const theme = themes.find((theme) => theme.name === card.theme);
  if (!theme) {
    throw new Error(`card ${card.id} starts in the theme ${JSON.stringify(card.theme)}, which the adapter does not ship`);
  }
  const context = await browser.newContext({
    viewport: { width: viewport.width, height: viewport.height },
    reducedMotion: "reduce",
  });
  await hermetic(context, refused);
  await seedTheme(context, theme);
  return context;
}
