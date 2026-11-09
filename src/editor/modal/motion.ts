/** Where motions go: functions from the text and a position to a new position. Mirrors
 * `crates/lgtm/src/edit/motion.rs`. */

import type { Buffer } from "./buffer";

/** The last char a normal-mode cursor can be on in `line`: before its newline, or the
 * newline itself on an empty line. */
export function lineMax(b: Buffer, line: number) {
  const start = b.lineStart(line);
  const end = b.lineEnd(line);
  return end > start ? end - 1 : start;
}

export const left = (b: Buffer, at: number) => (at > b.lineStart(b.lineOf(at)) ? at - 1 : at);

export const right = (b: Buffer, at: number) =>
  Math.max(Math.min(at + 1, lineMax(b, b.lineOf(at))), at);

/** `by` lines down (up when negative) from `at`, at `column` or `at`'s own, stepping over
 * the lines closed folds hide. Returns where and the column to keep. */
export function vertical(
  b: Buffer,
  at: number,
  by: number,
  column: number | undefined,
  hidden: [number, number][] = [],
): [number, number] {
  let line = b.lineOf(at);
  const col = column ?? at - b.lineStart(line);
  for (let n = 0; n < Math.abs(by); n++) {
    let next: number;
    if (by > 0) {
      const past = hidden.find(([s]) => s === line)?.[1] ?? line;
      next = Math.min(past + 1, b.lastLine());
    } else {
      const up = Math.max(line - 1, 0);
      next = hidden.find(([s, e]) => s < up && up <= e)?.[0] ?? up;
    }
    if (next === line) break;
    line = next;
  }
  return [Math.min(b.lineStart(line) + col, lineMax(b, line)), col];
}

export const firstNonBlank = (b: Buffer, line: number) => b.lineStart(line) + b.indent(line).length;

/** Where Ctrl-w deletes back to from the gap `at`: past spaces, then a word or a run of
 * punctuation. */
export function wordBackFrom(b: Buffer, at: number) {
  const start = b.lineStart(b.lineOf(at));
  let i = at;
  while (i > start && (b.char(i - 1) === " " || b.char(i - 1) === "\t")) i--;
  const c = b.char(i - 1);
  const kind = c === undefined ? undefined : classOf(c);
  while (i > start && classOf(b.char(i - 1) ?? "\n") === kind) i--;
  return i;
}

/** One level of indent: a tab when the file indents with tabs, else four spaces. */
export function indentUnit(b: Buffer) {
  for (let l = 0; l <= b.lastLine(); l++) {
    const i = b.indent(l);
    if (i) return i.startsWith("\t") ? "\t" : "    ";
  }
  return "    ";
}

/** What a char counts as, for word motions. */
export type Class = "space" | "eol" | "word" | "punct";

export function classOf(c: string): Class {
  if (c === "\n" || c === "\r") return "eol";
  if (/\s/.test(c)) return "space";
  if (/[\p{L}\p{N}_]/u.test(c)) return "word";
  return "punct";
}

function classAt(b: Buffer, i: number, big: boolean): Class {
  const k = classOf(b.char(i) ?? "\n");
  return big && k === "punct" ? "word" : k;
}

/** `w`: from `at` to just before the next word's start, as `[anchor, head]`. */
export function nextWordStart(b: Buffer, at: number, big: boolean): [number, number] {
  const n = b.len;
  if (n === 0) return [0, 0];
  const cls = (i: number) => classAt(b, i, big);
  let start = at;
  if (at + 1 < n && cls(at) !== cls(at + 1)) start = at + 1;
  let i = start;
  while (i < n && cls(i) === "eol") {
    i++;
    start = i;
  }
  if (i >= n) return [at, at];
  const kind = cls(i);
  while (i < n && cls(i) === kind) i++;
  while (i < n && cls(i) === "space") i++;
  return [start, i - 1];
}

/** `e`: from `at` to the end of the next word, as `[anchor, head]`. */
export function nextWordEnd(b: Buffer, at: number, big: boolean): [number, number] {
  const n = b.len;
  if (n === 0) return [0, 0];
  const cls = (i: number) => classAt(b, i, big);
  let start = at;
  if (at + 1 < n && cls(at) !== cls(at + 1)) start = at + 1;
  let i = start;
  while (i < n && (cls(i) === "space" || cls(i) === "eol")) i++;
  if (i >= n) return [at, at];
  const kind = cls(i);
  while (i + 1 < n && cls(i + 1) === kind) i++;
  return [start, i];
}

/** `b`: from `at` back to the start of the word before, as `[anchor, head]`. */
export function prevWordStart(b: Buffer, at: number, big: boolean): [number, number] {
  if (at === 0) return [0, 0];
  const cls = (i: number) => classAt(b, i, big);
  let start = at;
  if (cls(at) !== cls(at - 1)) start = at - 1;
  let i = start;
  while (i > 0 && (cls(i) === "space" || cls(i) === "eol")) i--;
  const kind = cls(i);
  while (i > 0 && cls(i - 1) === kind) i--;
  return [start, i];
}

/** `f` `t` `F` `T`: where `c` is on `at`'s line, forward or back, on it or next to it. */
export function findChar(
  b: Buffer,
  at: number,
  c: string,
  forward: boolean,
  till: boolean,
): number | undefined {
  const line = b.lineOf(at);
  const [start, end] = [b.lineStart(line), b.lineEnd(line)];
  if (forward) {
    for (let i = at + 1 + (till ? 1 : 0); i < end; i++) {
      if (b.char(i) === c) return till ? i - 1 : i;
    }
    return undefined;
  }
  for (let i = Math.max(at - (till ? 1 : 0), 0) - 1; i >= start; i--) {
    if (b.char(i) === c) return till ? i + 1 : i;
  }
  return undefined;
}

/** `x`: the selection's lines, newline and all, or one line more when it's that already. */
export function lineSelect(b: Buffer, from: number, to: number): [number, number] {
  const first = b.lineOf(from);
  let last = b.lineOf(to);
  const endOf = (line: number) => Math.min(b.lineEnd(line), b.lastChar());
  const whole = from === b.lineStart(first) && to === endOf(last);
  if (whole && last < b.lastLine()) last++;
  return [b.lineStart(first), endOf(last)];
}

/** The next match of `needle` after `at`, or before it going back, wrapping around, as
 * `[start, end]` with `end` past it. Matching ignores case unless `needle` has a capital. */
export function search(
  b: Buffer,
  at: number,
  needle: string,
  forward: boolean,
): [number, number] | undefined {
  const fold = needle === needle.toLowerCase();
  const hay = fold ? b.text.toLowerCase() : b.text;
  const pat = fold ? needle.toLowerCase() : needle;
  if (!pat || pat.length > hay.length) return undefined;
  const last = hay.length - pat.length;
  const hit = (i: number) => hay.startsWith(pat, i);
  const order: number[] = [];
  if (forward) {
    for (let i = at + 1; i <= last; i++) order.push(i);
    for (let i = 0; i <= Math.min(at, last); i++) order.push(i);
  } else {
    for (let i = Math.min(at, last + 1) - 1; i >= 0; i--) order.push(i);
    for (let i = last; i >= Math.min(at, last); i--) order.push(i);
  }
  const i = order.find(hit);
  return i === undefined ? undefined : [i, i + pat.length];
}
