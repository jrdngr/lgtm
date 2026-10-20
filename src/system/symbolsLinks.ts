import type { SymbolsDto } from "../bindings";
import type { Box } from "./layout";
import { type SymLayout, symKey } from "./symbolsLayout";

/** A link between two nodes, by key. */
export type Link = [from: string, to: string];

/** The links of node `key`: a symbol's calls both ways, or a chip's, which stands for its
 * component's symbols. With `chips`, an outside symbol at the other end is drawn as its chip. */
export function linksOf(dto: SymbolsDto, placed: SymLayout, key: string, chips: boolean): Link[] {
  const chipOf = new Map<number, string>();
  for (const c of placed.chips) for (const id of c.ids) chipOf.set(id, c.key);
  const chip = placed.chips.find((c) => c.key === key);
  const end = (id: number) =>
    chip?.ids.includes(id) ? chip.key : chips ? (chipOf.get(id) ?? symKey(id)) : symKey(id);
  const touches = chip ? (id: number) => chip.ids.includes(id) : (id: number) => symKey(id) === key;
  const seen = new Set<string>();
  const out: Link[] = [];
  for (const e of dto.edges) {
    if (!touches(e.from) && !touches(e.to)) continue;
    const link: Link = [end(e.from), end(e.to)];
    const k = link.join(">");
    if (link[0] === link[1] || seen.has(k)) continue;
    seen.add(k);
    out.push(link);
  }
  return out;
}

export interface Focus {
  pos: Map<string, Box>;
  links: Link[];
  width: number;
  height: number;
}

/** A collapsed graph's node size, the step between rows and the step between columns. */
export interface Dims {
  w: number;
  h: number;
  row: number;
  col: number;
}

const SYMBOL_DIMS: Dims = { w: 220, h: 32, row: 42, col: 300 };

/** The small graph `key` collapses into: its callers left, itself, its calls right. */
export function focusLayout(dto: SymbolsDto, placed: SymLayout, key: string): Focus {
  return collapse(linksOf(dto, placed, key, false), key, SYMBOL_DIMS);
}

/** `links` laid out around `key`: the nodes linking to it left, itself, the nodes it links to
 * right. A node linked both ways sits left. */
export function collapse(links: Link[], key: string, d: Dims): Focus {
  const callers = [...new Set(links.filter(([, b]) => b === key).map(([a]) => a))];
  const calls = [
    ...new Set(links.filter(([a, b]) => a === key && !callers.includes(b)).map(([, b]) => b)),
  ];
  const rows = Math.max(callers.length, calls.length, 1);
  const pos = new Map<string, Box>();
  const column = (list: string[], x: number) =>
    list.forEach((k, i) => {
      pos.set(k, { x, y: i * d.row + ((rows - list.length) * d.row) / 2, w: d.w, h: d.h });
    });
  const left = callers.length ? d.col : 0;
  column(callers, 0);
  pos.set(key, { x: left, y: ((rows - 1) * d.row) / 2, w: d.w, h: d.h });
  column(calls, left + d.col);
  return {
    pos,
    links,
    width: left + (calls.length ? d.col : 0) + d.w,
    height: rows * d.row - (d.row - d.h),
  };
}

const mid = (b: Box) => b.y + b.h / 2;

/** A wire from box `a` to box `b`: out of `a`'s right into `b`'s left when `b` is further
 * right, bowed out on the left when they share a column, and the mirror when `b` is left. */
export function wirePath(a: Box, b: Box): string {
  const ay = mid(a);
  const by = mid(b);
  if (Math.abs(a.x - b.x) < 20) {
    // Out to the left and back in, wider the farther apart, so it enters flat.
    const out = 36 + Math.min(64, Math.abs(by - ay) / 5);
    return `M${a.x} ${ay}C${a.x - out} ${ay} ${b.x - out} ${by} ${b.x} ${by}`;
  }
  const [ax, bx, dir] = b.x > a.x ? [a.x + a.w, b.x, 1] : [a.x, b.x + b.w, -1];
  const dx = Math.max(40, Math.abs(bx - ax) / 2) * dir;
  return `M${ax} ${ay}C${ax + dx} ${ay} ${bx - dx} ${by} ${bx} ${by}`;
}

/** One faint line per pair of files, or file and chip, that call each other. */
export function hairlines(dto: SymbolsDto, placed: SymLayout): string[] {
  const groupOf = new Map<number, Box & { key: string }>();
  const fileOf = new Map(placed.files.map((f) => [f.path, { ...f, key: f.path }]));
  for (const s of placed.syms) {
    const f = s.side === "in" ? fileOf.get(s.sym.path) : undefined;
    if (f) groupOf.set(s.sym.id, f);
  }
  for (const c of placed.chips) for (const id of c.ids) groupOf.set(id, c);
  const pairs = new Map<string, [Box, Box]>();
  for (const e of dto.edges) {
    const a = groupOf.get(e.from);
    const b = groupOf.get(e.to);
    if (!a || !b || a === b) continue;
    const k = a.key < b.key ? `${a.key}|${b.key}` : `${b.key}|${a.key}`;
    if (!pairs.has(k)) pairs.set(k, [a, b]);
  }
  return [...pairs.values()].map(([a, b]) => {
    if (Math.abs(a.x - b.x) < 20) {
      const ax = a.x + a.w;
      return `M${ax} ${mid(a)}C${ax + 40} ${mid(a)} ${ax + 40} ${mid(b)} ${ax} ${mid(b)}`;
    }
    const [l, r] = a.x < b.x ? [a, b] : [b, a];
    const lx = l.x + l.w;
    const dx = Math.max(30, (r.x - lx) / 2);
    return `M${lx} ${mid(l)}C${lx + dx} ${mid(l)} ${r.x - dx} ${mid(r)} ${r.x} ${mid(r)}`;
  });
}
