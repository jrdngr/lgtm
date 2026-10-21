import type { DisplayLine, FlatFile, ViewMode } from "../review/model";
import { findAll } from "./findText";

/** A match in the review: the display line it's on and which of that line's matches it is. */
export interface LineMatch {
  line: number;
  nth: number;
}

/** The code texts a line shows, in the order its `.code` elements render: one per side in
 * split mode, so a context row on both sides counts twice, as it shows. */
export function lineTexts(files: FlatFile[], line: DisplayLine, mode: ViewMode): string[] {
  if (line.kind === "context")
    return mode === "split" ? [line.line.text, line.line.text] : [line.line.text];
  if (line.kind !== "rows") return [];
  const rows = files[line.file]?.rows ?? [];
  const shown = mode === "unified" ? [line.rows[0]] : [line.left, line.right];
  return shown.flatMap((r) => (r === undefined || !rows[r] ? [] : [rows[r].text]));
}

/** Every match of `query` in the review's code, top to bottom. */
export function findInLines(
  files: FlatFile[],
  lines: DisplayLine[],
  mode: ViewMode,
  query: string,
  matchCase: boolean,
): LineMatch[] {
  if (!query) return [];
  const out: LineMatch[] = [];
  lines.forEach((l, line) => {
    let nth = 0;
    for (const text of lineTexts(files, l, mode)) {
      for (const _ of findAll(text, query, matchCase)) out.push({ line, nth: nth++ });
    }
  });
  return out;
}
