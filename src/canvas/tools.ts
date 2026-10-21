import type { ArrowEnd, EdgeStyle, NoteSize, ShapeKind, Swatch } from "../bindings";
import type { Pt } from "../system/edgePaths";
import type { Box } from "../system/layout";
import { BLUE, DASHED, DOTTED } from "../system/MapEdges";
import type { Paint } from "../system/mapWires";
import type { SEdge } from "../system/scene";
import { swatchHex } from "./palette";

export type Tool =
  | "select"
  | "rect"
  | "ellipse"
  | "diamond"
  | "arrow"
  | "line"
  | "text"
  | "note"
  | "frame";

export const SHAPE_TOOLS: Partial<Record<Tool, ShapeKind>> = {
  rect: "rect",
  ellipse: "ellipse",
  diamond: "diamond",
};

/** The tools by their keys. */
export const TOOL_KEYS: Record<string, Tool> = {
  v: "select",
  r: "rect",
  o: "ellipse",
  d: "diamond",
  a: "arrow",
  l: "line",
  t: "text",
  n: "note",
  f: "frame",
};

/** A press being dragged. */
export type Gesture =
  | { kind: "move"; start: Pt; at: Pt; parts: Set<string>; notes: Set<string>; items: string[] }
  | { kind: "marquee"; start: Pt; at: Pt; add: boolean }
  | { kind: "frame"; start: Pt; at: Pt }
  | { kind: "shape"; shape: ShapeKind; start: Pt; at: Pt }
  | { kind: "resize"; id: string; corner: number; start: Pt; at: Pt; from: Box }
  | { kind: "arrow"; from: ArrowEnd; at: Pt; free: boolean; head: boolean }
  | { kind: "end"; arrow: string; which: "from" | "to"; at: Pt; free: boolean }
  | { kind: "bend"; key: string; start: Pt; at: Pt; from: Pt }
  | { kind: "tag"; digit: string; at: Pt };

/** A note being typed. */
export interface Draft {
  id?: string;
  at: Pt;
  text: string;
  size: NoteSize;
  color: Swatch | null;
  sticky: boolean;
}

/** A shape's text being typed. */
export interface Label {
  id: string;
  text: string;
}

/** A drag shorter than this, in world units, is a click. */
export const SLOP = 3;
/** What a design edge draws in when its style gives no color. */
const NEW = "#a371f7";

export const ORIGIN: Pt = { x: 0, y: 0 };
export const round = (p: Pt): Pt => ({ x: Math.round(p.x), y: Math.round(p.y) });
export const centerOf = (b: Box): Pt => ({ x: b.x + b.w / 2, y: b.y + b.h / 2 });
export const rectOf = (a: Pt, b: Pt): Box => ({
  x: Math.min(a.x, b.x),
  y: Math.min(a.y, b.y),
  w: Math.abs(b.x - a.x),
  h: Math.abs(b.y - a.y),
});

export function edgePaint(style: EdgeStyle, e: SEdge | undefined): Paint {
  return {
    color: style.color ? swatchHex(style.color) : e?.status === "added" ? NEW : BLUE,
    dash: style.pattern === "dashed" ? DASHED : style.pattern === "dotted" ? DOTTED : undefined,
    width: style.weight === "thick" ? 3 : 1.6,
    route: style.route,
    bend: style.bend,
  };
}

export const defaultStyle = (): EdgeStyle => ({
  pattern: "solid",
  color: null,
  weight: "thin",
  route: "curved",
  bend: null,
  by: "user",
});
