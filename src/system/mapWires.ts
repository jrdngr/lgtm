import type { Box } from "./layout";
import { curve, type Layout } from "./layout";
import type { SEdge } from "./scene";
import { collapse, type Dims, type Focus, type Link } from "./symbolsLinks";

export const edgeKey = (e: { from: string; to: string }) => `${e.from}>${e.to}`;

/** How a bright wire is colored: blue when lit by a hover, the hot color for a tour, a trace
 * or the selected edge, and green or red for an edge the review adds or removes. */
export type Tone = "blue" | "hot" | "added" | "removed";

export type Pill =
  | { kind: "num"; text: string }
  | { kind: "added" | "removed"; text: string }
  | { kind: "label"; text: string; hot: boolean };

export interface Wire {
  e: SEdge;
  key: string;
  d: string;
  mid: { x: number; y: number };
  /** It joins a door, which gives it no detail. */
  door: boolean;
  /** Drawn bright with an arrow; only its hairline without. */
  tone?: Tone;
  /** Faded behind a hover. */
  faint?: boolean;
  /** The selected edge, drawn wider. */
  wide?: boolean;
  /** Lit, so drawn over the nodes. */
  over?: boolean;
  pill?: Pill;
  /** The pill's `mid` is its left end, or its right end, instead of its center. */
  anchor?: "start" | "end";
  /** Lit alone by a hover in a collapsed graph, its label shown whole. */
  alone?: boolean;
}

/** The edges a node draws when hovered or focused: those between it and other nodes the map
 * shows at rest, or else the ones only lit through it, like a container card's in a frame. */
export function edgesOf(edges: SEdge[], id: string): SEdge[] {
  const mine = edges.filter((e) => (e.from === id || e.to === id) && e.from !== e.to);
  const rest = mine.filter((e) => !e.detail);
  return rest.length ? rest : mine;
}

/** What a review-changed edge's pill says. */
export function statusLabel(e: SEdge) {
  if (e.status === "removed") return "removed";
  return `new · ${e.label ?? `${e.imports} ${e.imports === 1 ? "site" : "sites"}`}`;
}

/** A lit edge's pill: its status unless `quiet`, else its description. */
function litPill(e: SEdge, hot: boolean, quiet?: boolean): Pill | undefined {
  if (e.status && !quiet) return { kind: e.status, text: statusLabel(e) };
  return e.label ? { kind: "label", text: e.label, hot } : undefined;
}

interface RestState {
  /** Edges lit by a hover, by key. */
  hovered: Set<string>;
  /** Edges a tour or trace draws, some numbered, by key. */
  hot: Map<string, number | undefined>;
  selectedEdge?: string;
  /** Changed edges keep their color but show only their description, and only when lit. */
  quiet?: boolean;
}

/** The map at rest: every edge a hairline, bright only when hovered, hot, selected or, with the
 * Review lens, changed. A hover fades the changed edges it doesn't light. */
export function restWires(layout: Layout, edges: SEdge[], s: RestState): Wire[] {
  const box = new Map(layout.nodes.map((n) => [n.id, n]));
  const hovering = s.hovered.size > 0;
  return edges.flatMap((e) => {
    const a = box.get(e.from);
    const b = box.get(e.to);
    if (!a || !b) return [];
    const key = edgeKey(e);
    const hot = s.hot.has(key) || key === s.selectedEdge;
    const on = hot || s.hovered.has(key);
    if (e.detail && !on) return [];
    const n = s.hot.get(key);
    const pill: Pill | undefined =
      n !== undefined
        ? { kind: "num", text: String(n) }
        : on
          ? litPill(e, hot, s.quiet)
          : e.status && !hovering && !s.quiet
            ? { kind: e.status, text: statusLabel(e) }
            : undefined;
    return [
      {
        e,
        key,
        ...curve(a, b),
        door: !!(a.door || b.door),
        tone: e.status ?? (hot ? "hot" : on ? "blue" : undefined),
        faint: !on && hovering,
        wide: key === s.selectedEdge,
        over: on,
        pill,
      },
    ];
  });
}

/** A collapsed graph's straight run out of each neighbor, where its label sits, and the bend
 * from there to the center. */
const STUB = 214;
const BEND = 96;
/** Collapsed node size and spacing, with room between columns for a stub and its bend. */
const DIMS: Dims = { w: 172, h: 30, row: 40, col: 172 + STUB + BEND };
/** Between a neighbor and its label. */
const LABEL_GAP = 8;

/** The graph node `id` collapses into: the nodes that use it left, itself, what it uses right. */
export function mapFocus(edges: SEdge[], id: string): Focus {
  const links: Link[] = edgesOf(edges, id).map((e) => [e.from, e.to]);
  return collapse(links, id, DIMS);
}

/** A wire between the center and a neighbor: straight out of the neighbor for a label's room,
 * then bent to the center. The label's spot is by the neighbor. */
function stubWire(from: Box, to: Box, center: Box) {
  const nb = from === center ? to : from;
  const leftSide = nb.x < center.x;
  const ny = nb.y + nb.h / 2;
  const cy = center.y + center.h / 2;
  const nx = leftSide ? nb.x + nb.w : nb.x;
  const sx = leftSide ? nx + STUB : nx - STUB;
  const cx = leftSide ? center.x : center.x + center.w;
  const half = (cx - sx) / 2;
  const toCenter = `M${nx} ${ny}L${sx} ${ny}C${sx + half} ${ny} ${cx - half} ${cy} ${cx} ${cy}`;
  const toNb = `M${cx} ${cy}C${cx - half} ${cy} ${sx + half} ${ny} ${sx} ${ny}L${nx} ${ny}`;
  return {
    d: from === center ? toNb : toCenter,
    mid: { x: leftSide ? nx + LABEL_GAP : nx - LABEL_GAP, y: ny },
    anchor: leftSide ? ("start" as const) : ("end" as const),
  };
}

/** The collapsed graph's wires, all bright, each labeled by its neighbor; a neighbor linked
 * both ways gets one label for both. Hovering a neighbor or a wire fades the rest. */
export function focusWires(
  focus: Focus,
  center: string,
  edges: SEdge[],
  doors: Set<string>,
  hover: { node?: string; edge?: string },
  selectedEdge: string | undefined,
  quiet?: boolean,
): Wire[] {
  const byKey = new Map(edges.map((e) => [edgeKey(e), e]));
  const mid = focus.pos.get(center);
  if (!mid) return [];
  const labeled = new Set<string>();
  const both = new Map<string, string>();
  for (const [from, to] of focus.links) {
    const nb = from === center ? to : from;
    const label = byKey.get(`${from}>${to}`)?.label;
    if (label) both.set(nb, both.has(nb) ? `${both.get(nb)} · ${label}` : label);
  }
  const node = hover.node && hover.node !== center && focus.pos.has(hover.node) && hover.node;
  return focus.links.flatMap(([from, to]) => {
    const e = byKey.get(`${from}>${to}`);
    const a = focus.pos.get(from);
    const b = focus.pos.get(to);
    if (!e || !a || !b) return [];
    const key = edgeKey(e);
    const nb = from === center ? to : from;
    const alone = node ? node === nb : hover.edge === key;
    const faint = node ? !alone : !!hover.edge && !alone;
    const status = quiet ? undefined : e.status;
    const text = status ? undefined : both.get(nb);
    const first = !labeled.has(nb);
    labeled.add(nb);
    const pill: Pill | undefined = status
      ? { kind: status, text: statusLabel(e) }
      : text && first
        ? { kind: "label", text, hot: false }
        : undefined;
    return [
      {
        e,
        key,
        ...stubWire(a, b, mid),
        door: doors.has(from) || doors.has(to),
        tone: e.status ?? "blue",
        faint,
        alone,
        wide: key === selectedEdge,
        over: !faint,
        pill,
      },
    ];
  });
}
