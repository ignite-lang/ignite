import type { Viewport } from "../adapter";

// The width of one tile of a round's contact sheet: half the viewport, so
// body text stays readable on the sheet.
export function thumbWidth(viewport: Viewport): number {
  return Math.ceil(viewport.width / 2);
}

// Tiles per row of a sheet: as many as fit in about 3000 px.
export function tilesPerRow(viewport: Viewport): number {
  return Math.max(1, Math.floor(3000 / (thumbWidth(viewport) + 12)));
}
