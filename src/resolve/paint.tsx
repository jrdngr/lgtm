import type { ReactNode } from "react";
import { HIGHLIGHT_NAMES } from "../bindings";

/** A highlight class, or none, and how many UTF-16 units it covers. */
export type Segment = [number | null, number];

const hlClass = HIGHLIGHT_NAMES.map((n) => `hl-${n.replace(".", "-")}`);

/** A line's code in its highlight classes, with the runs `tints` names (in chars, from 0)
 * marked. Highlight segments count UTF-16 units. */
export function paint(text: string, hl: Segment[], tints: [number, number][]): ReactNode[] {
  // Each UTF-16 offset where a char starts, by char.
  const at: number[] = [];
  let u = 0;
  for (const ch of text) {
    at.push(u);
    u += ch.length;
  }
  at.push(u);
  const cuts = new Set<number>([0, text.length]);
  const classOf: (number | null)[] = new Array(text.length).fill(null);
  let pos = 0;
  for (const [cls, len] of hl) {
    for (let i = pos; i < Math.min(pos + len, text.length); i++) classOf[i] = cls;
    cuts.add(Math.min(pos, text.length));
    pos += len;
    cuts.add(Math.min(pos, text.length));
  }
  const marked = new Array<boolean>(text.length).fill(false);
  for (const [from, to] of tints) {
    const a = at[Math.min(from, at.length - 1)];
    const b = at[Math.min(to, at.length - 1)];
    for (let i = a; i < b; i++) marked[i] = true;
    cuts.add(a);
    cuts.add(b);
  }
  const edges = [...cuts].sort((a, b) => a - b);
  const out: ReactNode[] = [];
  for (let i = 0; i + 1 < edges.length; i++) {
    const [a, b] = [edges[i], edges[i + 1]];
    if (a === b) continue;
    const piece = text.slice(a, b);
    const cls = classOf[a];
    const inner = cls === null ? piece : <span className={hlClass[cls]}>{piece}</span>;
    out.push(marked[a] ? <mark key={a}>{inner}</mark> : <span key={a}>{inner}</span>);
  }
  return out;
}
