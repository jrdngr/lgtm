import type { Level } from "./scene";

/** A level the tab showed, and what was selected there when it left. */
export interface Place {
  level: Level;
  node?: string;
  edge?: string;
}

/** The places the tab went through, and which of them it shows. */
export interface Nav {
  stack: Place[];
  at: number;
}

export const NAV: Nav = { stack: [], at: -1 };

/** Leaves `from` for `to`, dropping the places forward of `from`. */
export function navPush(nav: Nav, from: Place, to: Place): Nav {
  const stack = [...nav.stack.slice(0, Math.max(0, nav.at)), from, to];
  return { stack, at: stack.length - 1 };
}

/** Steps `by` places back or forward, keeping `here` as the place left; the same nav when
 * there's nowhere to go. */
export function navStep(nav: Nav, here: Place, by: -1 | 1): Nav {
  const at = nav.at + by;
  if (nav.at < 0 || at < 0 || at >= nav.stack.length) return nav;
  const stack = nav.stack.map((p, i) => (i === nav.at ? here : p));
  return { stack, at };
}

export const sameLevel = (a: Level, b: Level) => JSON.stringify(a) === JSON.stringify(b);
