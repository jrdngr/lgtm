import type { FlatFile, ViewMode } from "./model";

const TAB = 4;
/** Gutter pixels before the code in a unified row, and in each side of a split row, from
 * review.css: risk, strip, ƒ marker, line numbers and sign. */
const GUTTER = { unified: 3 + 4 + 14 + 44 + 44 + 14, split: 3 + 4 + 14 + 44 + 14 };
/** Room after the longest line. */
const END = 2;

/** Columns `text` takes in a monospace font, with tabs stopping every four columns. */
export function columns(text: string): number {
  let col = 0;
  for (const ch of text) col = ch === "\t" ? col + TAB - (col % TAB) : col + 1;
  return col;
}

/** The CSS width that fits every row of `files` without clipping, for scrolling sideways. */
export function contentWidth(files: FlatFile[], mode: ViewMode): string {
  let longest = 0;
  for (const f of files) for (const r of f.rows) longest = Math.max(longest, columns(r.text));
  const side = `${GUTTER[mode]}px + ${longest + END}ch`;
  // Split rows also have a 1px border between the sides.
  return mode === "unified" ? `max(100%, calc(${side}))` : `max(100%, calc(2 * (${side}) + 1px))`;
}
