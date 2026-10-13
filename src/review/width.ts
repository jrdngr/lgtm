import type { FlatFile } from "./model";

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

/** The CSS width that fits every unified row of `files` without clipping. */
export function unifiedWidth(files: FlatFile[]): string {
  let longest = 0;
  for (const f of files) for (const r of f.rows) longest = Math.max(longest, columns(r.text));
  return `max(100%, calc(${GUTTER.unified}px + ${longest + END}ch))`;
}

/** The CSS widths that fit each side of every split row of `files`. Context rows are on
 * both sides. */
export function splitWidths(files: FlatFile[]): { left: string; right: string } {
  let left = 0;
  let right = 0;
  for (const f of files) {
    for (const r of f.rows) {
      const n = columns(r.text);
      if (r.kind !== "added") left = Math.max(left, n);
      if (r.kind !== "removed") right = Math.max(right, n);
    }
  }
  const side = (n: number) => `calc(${GUTTER.split}px + ${n + END}ch)`;
  return { left: side(left), right: side(right) };
}
