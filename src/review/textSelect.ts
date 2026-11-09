/** `v` selects text in the review with the editor's motions. The review's lines become a
 * read-only buffer, a line per display line: code lines hold their text, the new side's in
 * split view, and every other line is empty. Mirrors the TUI's `review/select.rs`. */

import { Buffer, Sel } from "../editor/modal/buffer";
import { firstNonBlank } from "../editor/modal/motion";
import type { DisplayLine, FlatFile, ViewMode } from "./model";

/** The row a code line shows its text from, or undefined when it shows none. */
export function shownRow(line: DisplayLine, mode: ViewMode): number | undefined {
  if (line.kind !== "rows") return undefined;
  return mode === "split" ? (line.right ?? line.left) : line.rows[0];
}

/** Each display line's text, a line each. */
export function lineTexts(files: FlatFile[], lines: DisplayLine[], mode: ViewMode): string {
  return lines
    .map((line) => {
      if (line.kind === "context") return line.line.text;
      const row = shownRow(line, mode);
      return row === undefined ? "" : files[line.file].rows[row].text;
    })
    .join("\n");
}

/** A buffer over the review's text with the cursor on `line`'s first non-blank char. */
export function startBuffer(text: string, line: number): Buffer {
  const b = new Buffer(text, "");
  b.sel = Sel.point(firstNonBlank(b, Math.min(line, b.lastLine())));
  return b;
}

/** The selected text. */
export function selectedText(b: Buffer): string {
  return b.text.slice(b.sel.from(), b.sel.to() + 1);
}

/** The cursor at `line`'s start or end, selecting nothing. */
export function lineEdge(b: Buffer, end: boolean) {
  const line = b.lineOf(b.sel.head);
  const [from, to] = [b.lineStart(line), b.lineEnd(line)];
  b.sel = Sel.point(end ? Math.max(to - 1, from) : from);
}

/** What's selected on line `n`, as chars `from..to` of it, and the cursor's char when it's on
 * the line. `head` past the line's text is its end. */
export function selectedOn(
  b: Buffer,
  n: number,
): { from: number; to: number; head?: number } | undefined {
  if (n > b.lastLine()) return undefined;
  const start = b.lineStart(n);
  const len = b.lineEnd(n) - start;
  const clamp = (i: number) => Math.min(Math.max(i, start), start + len) - start;
  const [from, to] = [clamp(b.sel.from()), clamp(b.sel.to() + 1)];
  const head = b.lineOf(b.sel.head) === n ? b.sel.head - start : undefined;
  return from < to || head !== undefined ? { from, to, head } : undefined;
}
