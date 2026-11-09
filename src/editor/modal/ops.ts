/** Edits on a buffer's selection. Each is one undo step; the caller checkpoints first.
 * Mirrors `crates/lgtm/src/edit/ops.rs`. */

import { type Buffer, Sel } from "./buffer";
import { indentUnit } from "./motion";

/** Yanked text, and whether it's whole lines, which paste as lines. */
export interface Register {
  text: string;
  lines: boolean;
}

/** The selection's text, and whether it's whole lines. */
export function yank(b: Buffer): Register {
  const [from, to] = [b.sel.from(), b.sel.to()];
  const text = b.slice(from, to + 1);
  return { text, lines: from === b.lineStart(b.lineOf(from)) && text.endsWith("\n") };
}

/** Removes the selection, leaving the cursor where it started. */
export function del(b: Buffer) {
  const [from, to] = [b.sel.from(), b.sel.to()];
  b.remove(from, Math.min(to + 1, b.len));
  b.sel = Sel.point(from);
  b.clamp();
}

/** Removes the selection to type in its place. Whole lines keep their last newline, so the
 * typing goes on a line of its own. Returns where typing goes. */
export function change(b: Buffer) {
  const from = b.sel.from();
  let to = b.sel.to() + 1;
  if (yank(b).lines) to--;
  b.remove(from, Math.min(to, b.len));
  return from;
}

/** Pastes `r` after the selection or before it, and selects what was pasted. */
export function paste(b: Buffer, r: Register, after: boolean) {
  if (!r.text) return;
  let at: number;
  if (r.lines && after) {
    const line = b.lineOf(b.sel.to());
    // The last line has no newline to paste after.
    if (b.lineEnd(line) >= b.len) b.insert(b.len, "\n");
    at = b.lineEnd(line) + 1;
  } else if (r.lines) at = b.lineStart(b.lineOf(b.sel.from()));
  else if (after) at = Math.min(b.sel.to() + 1, b.len);
  else at = b.sel.from();
  b.insert(at, r.text);
  b.sel = new Sel(at, at + r.text.length - 1);
  b.clamp();
}

function replaceSel(b: Buffer, f: (c: string) => string) {
  const [from, to] = [b.sel.from(), b.sel.to()];
  const text = [...b.slice(from, to + 1)].map(f).join("");
  b.remove(from, Math.min(to + 1, b.len));
  b.insert(from, text);
}

/** Replaces every char of the selection but newlines with `c`. */
export const replace = (b: Buffer, c: string) =>
  replaceSel(b, (x) => (x === "\n" || x === "\r" ? x : c));

/** Toggles the case of every letter in the selection. */
export function toggleCase(b: Buffer) {
  replaceSel(b, (c) => (c === c.toUpperCase() ? c.toLowerCase() : c.toUpperCase()));
  b.clamp();
}

/** Indents the selection's lines one level, or takes one level off. The selection stays on
 * the same text. */
export function indent(b: Buffer, more: boolean) {
  const unit = indentUnit(b);
  const spot = (p: number): [number, number] => [b.lineOf(p), p - b.lineStart(b.lineOf(p))];
  const [anchor, head] = [spot(b.sel.anchor), spot(b.sel.head)];
  const [first, last] = [b.lineOf(b.sel.from()), b.lineOf(b.sel.to())];
  const by: number[] = [];
  for (let line = first; line <= last; line++) {
    const start = b.lineStart(line);
    if (more) {
      if (b.lineEnd(line) > start) {
        b.insert(start, unit);
        by[line - first] = unit.length;
      } else by[line - first] = 0;
    } else {
      const ind = b.indent(line);
      const take = ind.startsWith("\t") ? 1 : (ind.slice(0, 4).match(/^ */)?.[0].length ?? 0);
      b.remove(start, start + take);
      by[line - first] = -take;
    }
  }
  const place = ([line, col]: [number, number]) =>
    b.lineStart(line) + Math.max(col + by[line - first], 0);
  b.sel = new Sel(place(anchor), place(head));
  b.clamp();
}

/** Joins the selection's lines, or its line and the next, with single spaces. */
export function join(b: Buffer) {
  const first = b.lineOf(b.sel.from());
  const last = Math.min(Math.max(b.lineOf(b.sel.to()), first + 1), b.lastLine());
  let at: number | undefined;
  for (let n = first; n < last; n++) {
    const end = b.lineEnd(first);
    if (end >= b.len) break;
    // The newline and the next line's indent become one space, or nothing before an empty
    // line.
    const next = first + 1;
    const skip = b.indent(next).length;
    const emptyNext = b.lineEnd(next) === b.lineStart(next) + skip;
    b.remove(end, end + 1 + skip);
    if (!emptyNext) b.insert(end, " ");
    at = end;
  }
  if (at !== undefined) {
    b.sel = Sel.point(at);
    b.clamp();
  }
}

/** The pair a surround char stands for, open and close. */
export function pair(c: string): [string, string] {
  switch (c) {
    case "(":
    case ")":
      return ["(", ")"];
    case "[":
    case "]":
      return ["[", "]"];
    case "{":
    case "}":
      return ["{", "}"];
    case "<":
    case ">":
      return ["<", ">"];
    default:
      return [c, c];
  }
}

/** Puts the pair for `c` around the selection, and selects it with them. */
export function surroundAdd(b: Buffer, c: string) {
  const [open, close] = pair(c);
  const [from, to] = [b.sel.from(), b.sel.to()];
  b.insert(Math.min(to + 1, b.len), close);
  b.insert(from, open);
  b.sel = new Sel(from, to + 2);
  b.clamp();
}

/** Where the pair for `c` closest around the selection is, open and close. */
export function surrounding(b: Buffer, c: string): [number, number] | undefined {
  const [open, close] = pair(c);
  const [from, to] = [b.sel.from(), b.sel.to()];
  if (open === close) {
    let before: number | undefined;
    for (let i = from - 1; i >= 0; i--) {
      if (b.char(i) === open) {
        before = i;
        break;
      }
    }
    if (before === undefined) return undefined;
    for (let i = to + 1; i < b.len; i++) if (b.char(i) === close) return [before, i];
    return undefined;
  }
  let depth = 0;
  let start: number | undefined;
  for (let i = from - 1; i >= 0; i--) {
    const x = b.char(i);
    if (x === close) depth++;
    else if (x === open && depth === 0) {
      start = i;
      break;
    } else if (x === open) depth--;
  }
  if (start === undefined) return undefined;
  depth = 0;
  for (let i = start + 1; i < b.len; i++) {
    const x = b.char(i);
    if (x === open) depth++;
    else if (x === close && depth === 0) return i > to || i >= from ? [start, i] : undefined;
    else if (x === close) depth--;
  }
  return undefined;
}

/** Takes off the pair for `c` around the selection. Returns whether there was one. */
export function surroundDelete(b: Buffer, c: string) {
  const found = surrounding(b, c);
  if (!found) return false;
  const [open, close] = found;
  b.remove(close, close + 1);
  b.remove(open, open + 1);
  b.sel = new Sel(Math.max(b.sel.anchor - 1, 0), Math.max(b.sel.head - 1, 0));
  b.clamp();
  return true;
}

/** Swaps the pair for `from` around the selection for the pair for `to`. */
export function surroundReplace(b: Buffer, from: string, to: string) {
  const found = surrounding(b, from);
  if (!found) return false;
  const [open, close] = found;
  const [o, c] = pair(to);
  b.remove(close, close + 1);
  b.insert(close, c);
  b.remove(open, open + 1);
  b.insert(open, o);
  return true;
}
