import type { Box, Layout, Placed } from "../system/layout";
import type { SEdge } from "../system/scene";
import { swatchHex } from "./palette";
import type { Canvas, View } from "./view";

/** Parts being dragged, by how far. */
export interface Drag {
  ids: Set<string>;
  dx: number;
  dy: number;
}

export interface Arranged {
  layout: Layout;
  /** Each part's and shape's box, by id. */
  boxes: Map<string, Box>;
  /** Parts without a position in a view the user has arranged: new since. */
  fresh: Set<string>;
}

/** Between the boundary and the parts it holds, as the automatic layout leaves it. */
const INSET = 36;
/** Around the world's content. */
const PAD = 40;
/** Kept clear around a part placed in open space. */
const CLEAR = 28;

const meets = (a: Box, b: Box, gap: number) =>
  a.x < b.x + b.w + gap && b.x < a.x + a.w + gap && a.y < b.y + b.h + gap && b.y < a.y + a.h + gap;

/** The nearest spot to (`x`, `y`) where a `w` by `h` box clears `taken`, searching outward. */
function openSpot(x: number, y: number, w: number, h: number, taken: Box[]) {
  const step = 30;
  for (let ring = 0; ring < 60; ring++) {
    for (let i = -ring; i <= ring; i++) {
      for (const [dx, dy] of [
        [i, -ring],
        [i, ring],
        [-ring, i],
        [ring, i],
      ]) {
        const box = { x: Math.max(0, x + dx * step), y: Math.max(0, y + dy * step), w, h };
        if (!taken.some((t) => meets(t, box, CLEAR))) return { x: box.x, y: box.y };
      }
    }
  }
  return { x, y };
}

const union = (boxes: Box[]): Box => {
  const x = Math.min(...boxes.map((b) => b.x));
  const y = Math.min(...boxes.map((b) => b.y));
  return {
    x,
    y,
    w: Math.max(...boxes.map((b) => b.x + b.w)) - x,
    h: Math.max(...boxes.map((b) => b.y + b.h)) - y,
  };
};

const inside = (b: Box, outer: Box) => {
  const cx = b.x + b.w / 2;
  const cy = b.y + b.h / 2;
  return cx > outer.x && cx < outer.x + outer.w && cy > outer.y && cy < outer.y + outer.h;
};

/** The automatic layout `base` with the view's positions, colors and a drag applied, and the
 * view's shapes. Parts
 * the view has no position for keep their automatic spot, or once the user has arranged the
 * view, go to open space near the parts they link to. Arranged, a view drops its tier bands,
 * and its boundary follows the parts that started inside it. `extra` boxes (groups, notes,
 * arrows) widen the world. */
export function arrange(
  base: Layout,
  edges: SEdge[],
  v: View,
  colors: Canvas["colors"],
  drag?: Drag,
  extra: Box[] = [],
): Arranged {
  const positioned = Object.keys(v.positions).length > 0;
  const fresh = new Set<string>();
  const placed = new Map<string, Placed>();
  const unplaced: Placed[] = [];
  for (const n of base.nodes) {
    const at = v.positions[n.id];
    if (at) placed.set(n.id, { ...n, x: at.x, y: at.y });
    else if (positioned) unplaced.push(n);
    else placed.set(n.id, n);
  }
  for (const n of unplaced) {
    const linked = edges
      .flatMap((e) => (e.from === n.id ? [e.to] : e.to === n.id ? [e.from] : []))
      .map((id) => placed.get(id))
      .filter((b): b is Placed => !!b);
    let start = { x: n.x, y: n.y };
    if (v.frozen && linked.length) {
      start = {
        x: linked.reduce((s, b) => s + b.x, 0) / linked.length,
        y: linked.reduce((s, b) => s + b.y + b.h, 0) / linked.length + CLEAR * 2,
      };
    }
    const at = openSpot(start.x, start.y, n.w, n.h, [...placed.values()]);
    placed.set(n.id, { ...n, ...at });
    if (v.frozen) fresh.add(n.id);
  }
  const nodes = base.nodes.map((n) => {
    let p = placed.get(n.id) ?? n;
    if (drag?.ids.has(n.id))
      p = { ...p, x: Math.max(0, p.x + drag.dx), y: Math.max(0, p.y + drag.dy) };
    const paint = colors[n.id];
    return paint ? { ...p, paint: swatchHex(paint) } : p;
  });
  const boxes = new Map<string, Box>(nodes.map((n) => [n.id, n]));
  const shapes = v.shapes.map((s): Box => {
    const off = drag?.ids.has(s.id) ? { x: drag.dx, y: drag.dy } : { x: 0, y: 0 };
    return { x: s.at.x + off.x, y: s.at.y + off.y, w: s.w, h: s.h };
  });
  for (const [i, s] of v.shapes.entries()) boxes.set(s.id, shapes[i]);
  const moved = positioned || !!drag;
  let boundary = base.boundary;
  if (moved && base.boundary) {
    const b = base.boundary;
    const held = nodes.filter((n) => inside(base.nodes.find((x) => x.id === n.id) ?? n, b));
    if (held.length) {
      const u = union(held);
      boundary = {
        ...b,
        x: u.x - INSET,
        y: u.y - INSET,
        w: u.w + INSET * 2,
        h: u.h + INSET * 2,
      };
    }
  }
  const all = [...nodes, ...(boundary ? [boundary] : []), ...shapes, ...extra];
  const far = all.length ? union(all) : { x: 0, y: 0, w: 0, h: 0 };
  return {
    layout: {
      nodes,
      tiers: moved ? [] : base.tiers,
      boundary,
      width: Math.max(moved ? 0 : base.width, far.x + far.w + PAD),
      height: Math.max(moved ? 0 : base.height, far.y + far.h + PAD),
      left: Math.min(0, far.x - PAD),
      top: Math.min(0, far.y - PAD),
    },
    boxes,
    fresh,
  };
}

/** `v` with every part where `layout` draws it, so nothing moves on its own from here. */
export function freeze(v: View, layout: Layout): View {
  return {
    ...v,
    frozen: true,
    positions: Object.fromEntries(layout.nodes.map((n) => [n.id, { x: n.x, y: n.y }])),
  };
}
