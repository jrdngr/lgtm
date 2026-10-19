import { curve, type Layout } from "./layout";
import type { SEdge } from "./scene";
import { collapse, type Dims, type Focus, type Link, wireMid, wirePath } from "./symbolsLinks";

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
  pill?: Pill;
}

/** What a review-changed edge's pill says. */
export function statusLabel(e: SEdge) {
  if (e.status === "removed") return "removed";
  return `new · ${e.label ?? `${e.imports} ${e.imports === 1 ? "site" : "sites"}`}`;
}

/** A lit edge's pill: its status, or its description. */
function litPill(e: SEdge, hot: boolean): Pill | undefined {
  if (e.status) return { kind: e.status, text: statusLabel(e) };
  return e.label ? { kind: "label", text: e.label, hot } : undefined;
}

interface RestState {
  /** Edges lit by a hover, by key. */
  hovered: Set<string>;
  /** Edges a tour or trace draws, some numbered, by key. */
  hot: Map<string, number | undefined>;
  selectedEdge?: string;
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
    const n = s.hot.get(key);
    const pill: Pill | undefined =
      n !== undefined
        ? { kind: "num", text: String(n) }
        : on
          ? litPill(e, hot)
          : e.status && !hovering
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
        pill,
      },
    ];
  });
}

/** Collapsed node size and spacing, with room between columns for labels. */
const DIMS: Dims = { w: 190, h: 34, row: 46, col: 400 };

/** The graph node `id` collapses into: the nodes that use it left, itself, what it uses right. */
export function mapFocus(edges: SEdge[], id: string): Focus {
  const links: Link[] = edges
    .filter((e) => (e.from === id || e.to === id) && e.from !== e.to)
    .map((e) => [e.from, e.to]);
  return collapse(links, id, DIMS);
}

/** The collapsed graph's wires, all bright; a hovered neighbor fades the ones it isn't on. */
export function focusWires(
  focus: Focus,
  edges: SEdge[],
  doors: Set<string>,
  hover: string | undefined,
  selectedEdge: string | undefined,
): Wire[] {
  const byKey = new Map(edges.map((e) => [edgeKey(e), e]));
  return focus.links.flatMap(([from, to]) => {
    const e = byKey.get(`${from}>${to}`);
    const a = focus.pos.get(from);
    const b = focus.pos.get(to);
    if (!e || !a || !b) return [];
    const key = edgeKey(e);
    const faint = !!hover && hover !== from && hover !== to && focus.pos.has(hover);
    return [
      {
        e,
        key,
        d: wirePath(a, b),
        mid: wireMid(a, b),
        door: doors.has(from) || doors.has(to),
        tone: e.status ?? "blue",
        faint,
        wide: key === selectedEdge,
        pill: litPill(e, false),
      },
    ];
  });
}
