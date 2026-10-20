import type { Frame, NodeKind, Scene, SNode } from "./scene";

export interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface Placed extends SNode, Box {}

export interface Layout {
  /** Group frames come before the cards inside them, so they draw under them. */
  nodes: Placed[];
  /** Tier bands: a tier's name and the rows it spans. */
  tiers: (Box & { label: string })[];
  boundary?: Box & { name: string; color?: string };
  width: number;
  height: number;
}

const SIZE: Record<NodeKind, [number, number]> = {
  person: [150, 100],
  system: [340, 240],
  external: [170, 70],
  container: [210, 150],
  component: [140, 48],
  group: [300, 110],
};
/** Another container at a components view's edge. */
const DOOR: [number, number] = [150, 48];
/** A container's card in a group frame. */
const SMALL: [number, number] = [180, 60];

const GAP_X = 30;
const GAP_Y = 44;
const PAD = 40;
/** Between a side column and the boundary. */
const COLUMN_GAP = 110;
/** Between nodes in a side column, and between doors. */
const COLUMN_Y = 56;
const DOOR_Y = 14;
/** Between the columns a long side column wraps into. */
const WRAP_X = 24;
/** How many rows a side column may run past the boundary, above and below together. */
const OVERHANG = 2;
/** Room for tier names left of the boundary's rows. */
const TIER_W = 90;
/** Between the boundary and the rows it holds. */
const INSET = 36;
/** Around a tier band's rows. */
const BAND = 14;
/** Between packed nodes or frames. */
const PACK_GAP = 24;

/** A system card's header, padding and border, above its blocks. */
const SYSTEM_TOP = 72;
/** A block row and the gap below it. */
const BLOCK_ROW = 34;
/** A group card's header and padding, above its blocks, two a row. */
const GROUP_TOP = 60;

/** A frame's header, padding, and the gap between its cards. */
const FRAME_TOP = 44;
const FRAME_PAD = 14;
const CARD_GAP = 12;

/** A system card tall enough for its blocks, three a row. */
function systemSize(n: SNode): [number, number] {
  const [w, h] = SIZE.system;
  const count = n.blocks?.length ?? 0;
  return [w, Math.max(h, SYSTEM_TOP + Math.ceil(count / 3) * BLOCK_ROW)];
}

function groupSize(n: SNode): [number, number] {
  const [w, h] = SIZE.group;
  const count = n.blocks?.length ?? 0;
  return [w, Math.max(h, GROUP_TOP + Math.ceil(count / 2) * BLOCK_ROW)];
}

/** A design component's card: its name, then a line per interface. */
const LINE = 18;
const linesSize = (n: SNode): [number, number] => [200, 44 + (n.lines?.length ?? 0) * LINE];

const size = (n: SNode): [number, number] =>
  n.lines?.length && !n.door
    ? linesSize(n)
    : n.door
      ? DOOR
      : n.small
        ? SMALL
        : n.kind === "system"
          ? systemSize(n)
          : n.kind === "group"
            ? groupSize(n)
            : SIZE[n.kind];

const gapX = (nodes: SNode[]) => (nodes.some((n) => n.kind === "container") ? 60 : GAP_X);

const rowWidth = (nodes: SNode[]) =>
  nodes.reduce((w, n) => w + size(n)[0], 0) + gapX(nodes) * Math.max(0, nodes.length - 1);

const columnGap = (nodes: SNode[]) => (nodes.every((n) => n.door) ? DOOR_Y : COLUMN_Y);

const columnHeight = (nodes: SNode[]) =>
  nodes.reduce((h, n) => h + size(n)[1], 0) + columnGap(nodes) * Math.max(0, nodes.length - 1);

/** A side column split into columns about as tall as the boundary, running at most
 * `OVERHANG` rows past it, the first column nearest the boundary. */
function wrapColumn(nodes: SNode[], centerH: number): SNode[][] {
  if (!nodes.length) return [];
  const step = Math.max(...nodes.map((n) => size(n)[1])) + columnGap(nodes);
  const rows = Math.max(1, Math.round((centerH + columnGap(nodes)) / step)) + OVERHANG;
  const out: SNode[][] = [];
  for (let i = 0; i < nodes.length; i += rows) out.push(nodes.slice(i, i + rows));
  return out;
}

const sideWidth = (cols: SNode[][]) =>
  cols.reduce((w, c) => w + Math.max(...c.map((n) => size(n)[0])), 0) +
  WRAP_X * Math.max(0, cols.length - 1);

/** What sits inside the boundary, placed from (0, 0). */
interface Center {
  nodes: Placed[];
  tiers: Layout["tiers"];
  w: number;
  h: number;
}

/** Rows top to bottom, each centered, in tier bands when they have names. */
function rowsCenter(scene: Scene): Center {
  const tiered = scene.inside.some((r) => r.label);
  const tierLeft = tiered ? TIER_W : 0;
  const rowsW = Math.max(0, ...scene.inside.map((r) => rowWidth(r.nodes)));
  const rowH = (nodes: SNode[]) => Math.max(...nodes.map((n) => size(n)[1]));
  const nodes: Placed[] = [];
  const tiers: Layout["tiers"] = [];
  let y = 0;
  for (const row of scene.inside) {
    const h = rowH(row.nodes);
    let nx = tierLeft + (rowsW - rowWidth(row.nodes)) / 2;
    for (const n of row.nodes) {
      const [w, nh] = size(n);
      nodes.push({ ...n, x: nx, y: y + (h - nh) / 2, w, h: nh });
      nx += w + gapX(row.nodes);
    }
    const band = { x: -BAND, y: y - BAND, w: tierLeft + rowsW + BAND * 2, h: h + BAND * 2 };
    const last = tiers.at(-1);
    if (row.label) tiers.push({ label: row.label, ...band });
    else if (last && tiered) last.h = band.y + band.h - last.y;
    y += h + GAP_Y;
  }
  return { nodes, tiers, w: rowsW + tierLeft, h: Math.max(0, y - GAP_Y) };
}

/** Boxes of the sizes given packed into `cols` columns, each into the shortest so far. */
function packBoxes(sizes: [number, number][], cols: number) {
  const colW = Math.max(0, ...sizes.map(([w]) => w));
  const heights = new Array(Math.max(1, Math.min(cols, sizes.length))).fill(0);
  const boxes = sizes.map(([w, h]) => {
    const c = heights.indexOf(Math.min(...heights));
    const box = { x: c * (colW + PACK_GAP), y: heights[c], w, h };
    heights[c] += h + PACK_GAP;
    return box;
  });
  return {
    boxes,
    w: heights.length * (colW + PACK_GAP) - PACK_GAP,
    h: Math.max(...heights) - PACK_GAP,
  };
}

function packCenter(scene: Scene, cols: number): Center {
  const all = scene.inside.flatMap((r) => r.nodes);
  const p = packBoxes(all.map(size), cols);
  return {
    nodes: all.map((n, i) => ({ ...n, ...p.boxes[i] })),
    tiers: [],
    w: p.w,
    h: p.h,
  };
}

function frameSize(f: Frame): [number, number] {
  const rows = Math.ceil(f.nodes.length / 2);
  const cols = Math.min(2, f.nodes.length);
  return [
    FRAME_PAD * 2 + cols * SMALL[0] + (cols - 1) * CARD_GAP,
    FRAME_TOP + rows * SMALL[1] + (rows - 1) * CARD_GAP + FRAME_PAD,
  ];
}

/** Frames packed in about as many columns as rows, each holding its cards two a row. */
function framesCenter(frames: Frame[]): Center {
  const cols = Math.max(1, Math.min(frames.length, Math.round(Math.sqrt(frames.length * 1.3))));
  const p = packBoxes(frames.map(frameSize), cols);
  const nodes: Placed[] = [];
  const cards: Placed[] = [];
  frames.forEach((f, i) => {
    const b = p.boxes[i];
    nodes.push({ ...f.node, ...b });
    f.nodes.forEach((n, j) => {
      cards.push({
        ...n,
        x: b.x + FRAME_PAD + (j % 2) * (SMALL[0] + CARD_GAP),
        y: b.y + FRAME_TOP + Math.floor(j / 2) * (SMALL[1] + CARD_GAP),
        w: SMALL[0],
        h: SMALL[1],
      });
    });
  });
  return { nodes: [...nodes, ...cards], tiers: [], w: p.w, h: p.h };
}

/** Places the scene's inside within the boundary, as rows, packed columns or group frames,
 * with its side columns left and right of it. */
export function layout(scene: Scene): Layout {
  const center = scene.frames
    ? framesCenter(scene.frames)
    : scene.pack
      ? packCenter(scene, scene.pack)
      : rowsCenter(scene);
  const framed = scene.boundary !== undefined && center.nodes.length > 0;
  const inset = framed ? INSET : 0;
  const centerW = center.w + inset * 2;
  const centerH = center.h + inset * 2;
  const left = wrapColumn(scene.left, centerH);
  const right = wrapColumn(scene.right, centerH);
  const leftW = sideWidth(left);
  const rightW = sideWidth(right);
  const height = Math.max(centerH, ...[...left, ...right].map((c) => columnHeight(c))) + PAD * 2;
  const nodes: Placed[] = [];

  const column = (col: SNode[], x: number) => {
    const w = Math.max(...col.map((n) => size(n)[0]));
    let y = (height - columnHeight(col)) / 2;
    for (const n of col) {
      const [nw, nh] = size(n);
      nodes.push({ ...n, x: x + (w - nw) / 2, y, w: nw, h: nh });
      y += nh + columnGap(col);
    }
    return w + WRAP_X;
  };

  let x = PAD;
  if (left.length) {
    let lx = x;
    for (const col of [...left].reverse()) lx += column(col, lx);
    x += leftW + COLUMN_GAP;
  }
  const cx = x;
  const cy = (height - centerH) / 2;
  const ox = cx + inset;
  const oy = cy + inset;
  for (const n of center.nodes) nodes.push({ ...n, x: n.x + ox, y: n.y + oy });
  const tiers = center.tiers.map((t) => ({ ...t, x: t.x + ox, y: t.y + oy }));
  const boundary =
    framed && scene.boundary !== undefined
      ? { name: scene.boundary, color: scene.color, x: cx, y: cy, w: centerW, h: centerH }
      : undefined;
  x += centerW;
  if (right.length) {
    x += COLUMN_GAP;
    let rx = x;
    for (const col of right) rx += column(col, rx);
    x += rightW;
  }
  return { nodes, tiers, boundary, width: x + PAD, height };
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
