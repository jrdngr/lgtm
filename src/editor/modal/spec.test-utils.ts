/** Key cases for tests, as `crates/lgtm/src/edit/spec.rs` reads them: text with its
 * selection marked, keys to press, and the text and selection they should leave.
 *
 * `[` and `]` mark a selection, its cursor on the last char unless `<` follows `[`. `|`
 * marks the insert-mode gap. Keys are chars, with `<name>` for named keys like `<esc>`,
 * `<ret>` or `<C-d>`. */

import { Buffer, Sel } from "./buffer";
import type { Key } from "./keymap";
import { Keys, type Mode } from "./keys";

export function buffer(marked: string, name = "t.txt"): Buffer {
  let text = "";
  let start: number | undefined;
  let end: number | undefined;
  let back = false;
  let gap: number | undefined;
  const chars = [...marked];
  for (let i = 0; i < chars.length; i++) {
    const c = chars[i];
    if (c === "[") {
      start = text.length;
      if (chars[i + 1] === "<") {
        i++;
        back = true;
      }
    } else if (c === "]") end = text.length - 1;
    else if (c === "|") gap = text.length;
    else text += c;
  }
  const b = new Buffer(text, name);
  if (gap !== undefined) b.sel = Sel.point(gap);
  else if (start !== undefined && end !== undefined)
    b.sel = back ? new Sel(end, start) : new Sel(start, end);
  return b;
}

/** The buffer's text with its selection marked as `buffer` reads it. */
export function marked(b: Buffer, mode: Mode): string {
  let out = "";
  const text = b.text;
  for (let i = 0; i < text.length; i++) {
    if (mode === "insert") {
      if (i === b.sel.head) out += "|";
    } else if (i === b.sel.from()) {
      out += "[";
      if (b.sel.head < b.sel.anchor) out += "<";
    }
    out += text[i];
    if (mode !== "insert" && i === b.sel.to()) out += "]";
  }
  if (mode === "insert" && b.sel.head >= text.length) out += "|";
  return out;
}

const NAMED: Record<string, string> = {
  esc: "Escape",
  ret: "Enter",
  bs: "Backspace",
  del: "Delete",
  tab: "Tab",
  space: " ",
  lt: "<",
  left: "ArrowLeft",
  right: "ArrowRight",
  up: "ArrowUp",
  down: "ArrowDown",
  home: "Home",
  end: "End",
};

export function keys(spec: string): Key[] {
  const out: Key[] = [];
  for (let i = 0; i < spec.length; i++) {
    const c = spec[i];
    if (c !== "<") {
      out.push({ key: c });
      continue;
    }
    const close = spec.indexOf(">", i);
    const name = spec.slice(i + 1, close);
    i = close;
    if (name === "S-tab") {
      out.push({ key: "Tab", shift: true });
      continue;
    }
    const [mod, base] = name.includes("-") ? name.split("-", 2) : ["", name];
    out.push({ key: NAMED[base] ?? base, ctrl: mod === "C", alt: mod === "A" });
  }
  return out;
}

/** Presses `spec`'s keys on `start` and gives the marked result. */
export function run(start: string, spec: string): string {
  const b = buffer(start);
  const k = new Keys();
  for (const key of keys(spec)) k.press(b, key, { height: 10, hidden: [] });
  return marked(b, k.mode);
}
