/** Text objects for `mi` and `ma`. Mirrors `crates/lgtm/src/edit/objects.rs`; the function
 * object and syntax nodes come from the syntax tree, through `Syntax`. */

import type { Buffer } from "./buffer";
import { type Class, classOf } from "./motion";
import { surrounding } from "./ops";

/** The chars of the object `c` around the selection, inside or around it, as `[from, to]`
 * with `to` included. The function object isn't here: it needs the syntax tree. */
export function object(b: Buffer, c: string, around: boolean): [number, number] | undefined {
  if (c === "w" || c === "W") return word(b, c === "W", around);
  if (!"()[]{}<>\"'`".includes(c)) return undefined;
  const found = surrounding(b, c);
  if (!found) return undefined;
  const [open, close] = found;
  if (around) return [open, close];
  return close > open + 1 ? [open + 1, close - 1] : undefined;
}

/** The word under the cursor, and with `around` the spaces after it, or before it when none
 * follow. */
function word(b: Buffer, big: boolean, around: boolean): [number, number] | undefined {
  const at = b.sel.head;
  const kind = (i: number): Class | undefined => {
    const c = b.char(i);
    if (c === undefined) return undefined;
    const k = classOf(c);
    return big && k === "punct" ? "word" : k;
  };
  const here = kind(at);
  if (here === undefined || here === "eol") return undefined;
  let [from, to] = [at, at];
  while (from > 0 && kind(from - 1) === here) from--;
  while (kind(to + 1) === here) to++;
  if (around) {
    let end = to;
    while (kind(end + 1) === "space") end++;
    if (end > to) to = end;
    else while (from > 0 && kind(from - 1) === "space") from--;
  }
  return [from, to];
}
