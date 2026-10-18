import type { NodeKind, Scene, SNode } from "./scene";

export interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface Placed extends SNode, Box {}

export interface Layout {
  nodes: Placed[];
  /** Tier names, where they start and the y of their row's middle. */
  tiers: { label: string; x: number; y: number }[];
  boundary?: Box & { name: string };
  width: number;
  height: number;
}

const SIZE: Record<NodeKind, [number, number]> = {
  person: [180, 84],
  system: [320, 140],
  external: [200, 92],
  container: [240, 124],
  component: [160, 88],
};

const GAP_X = 48;
const GAP_Y = 72;
const PAD = 40;
/** Room for tier names left of the boundary's rows. */
const TIER_W = 96;
/** Between the boundary and the rows it holds. */
const INSET = 28;

/** Places the scene's rows top to bottom, each centered. */
export function layout(scene: Scene): Layout {
  const groups = [scene.above, scene.inside, scene.below];
  const tiered = scene.inside.some((r) => r.label);
  const rowWidth = (nodes: SNode[]) =>
    nodes.reduce((w, n) => w + SIZE[n.kind][0], 0) + GAP_X * Math.max(0, nodes.length - 1);
  const insideLeft = tiered ? TIER_W : 0;
  const inner = Math.max(0, ...scene.inside.map((r) => rowWidth(r.nodes))) + insideLeft;
  const framed = scene.boundary !== undefined && scene.inside.length > 0;
  const width =
    Math.max(
      inner + (framed ? INSET * 2 : 0),
      ...[...scene.above, ...scene.below].map((r) => rowWidth(r.nodes)),
    ) +
    PAD * 2;
  const nodes: Placed[] = [];
  const tiers: Layout["tiers"] = [];
  let boundary: Layout["boundary"];
  let y = PAD;
  // The boundary's rows sit right of the tier names, centered in what's left.
  const x0 = (width - inner) / 2;
  groups.forEach((rows, g) => {
    if (!rows.length) return;
    const inside = g === 1;
    const top = y;
    if (inside && framed) y += INSET;
    for (const row of rows) {
      const h = Math.max(...row.nodes.map((n) => SIZE[n.kind][1]));
      const [from, span] = inside ? [x0 + insideLeft, inner - insideLeft] : [0, width];
      let x = from + (span - rowWidth(row.nodes)) / 2;
      for (const n of row.nodes) {
        const [w, nh] = SIZE[n.kind];
        nodes.push({ ...n, x, y: y + (h - nh) / 2, w, h: nh });
        x += w + GAP_X;
      }
      if (inside && row.label) tiers.push({ label: row.label, x: x0, y: y + h / 2 });
      y += h + GAP_Y;
    }
    if (inside && framed && scene.boundary !== undefined) {
      const bottom = y - GAP_Y + INSET;
      boundary = {
        name: scene.boundary,
        x: x0 - INSET,
        y: top,
        w: inner + INSET * 2,
        h: bottom - top,
      };
      y = bottom + GAP_Y;
    }
  });
  return { nodes, tiers, boundary, width, height: y - GAP_Y + PAD };
}

/** Where the line from `a`'s center to `b`'s center leaves `a`'s box. */
function exit(a: Box, b: Box) {
  const ax = a.x + a.w / 2;
  const ay = a.y + a.h / 2;
  const dx = b.x + b.w / 2 - ax;
  const dy = b.y + b.h / 2 - ay;
  const t = Math.min(
    dx ? a.w / 2 / Math.abs(dx) : Number.POSITIVE_INFINITY,
    dy ? a.h / 2 / Math.abs(dy) : Number.POSITIVE_INFINITY,
  );
  return { x: ax + dx * t, y: ay + dy * t };
}

/** A curve between two boxes' edges, bowed to the right of its direction so that edges
 * both ways between two boxes stay apart, with the point halfway along it. */
export function curve(a: Box, b: Box) {
  const p = exit(a, b);
  const q = exit(b, a);
  const dx = q.x - p.x;
  const dy = q.y - p.y;
  const len = Math.hypot(dx, dy) || 1;
  const bend = Math.min(60, Math.max(12, len * 0.12));
  const cx = (p.x + q.x) / 2 - (dy / len) * bend;
  const cy = (p.y + q.y) / 2 + (dx / len) * bend;
  return {
    d: `M${p.x} ${p.y}Q${cx} ${cy} ${q.x} ${q.y}`,
    mid: { x: (p.x + 2 * cx + q.x) / 4, y: (p.y + 2 * cy + q.y) / 4 },
  };
}
