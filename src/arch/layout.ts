import type { ElkNode } from "elkjs/lib/elk-api.js";
import { getElk, type Point, type Rect } from "../graph/layout";

export const NODE_W = 150;
export const NODE_H = 48;
/** Room above a container's components for its header band. */
const HEADER = 34;
/** A group's ports are points on its sides. */
const PORT = 1;

export interface LayoutInput {
  /** Boxes around nodes, like containers around their components. */
  groups: { id: string; children: string[] }[];
  /** Every node, in or out of a group. */
  nodes: string[];
  edges: { key: string; from: string; to: string }[];
}

export interface Placed {
  groups: Map<string, Rect>;
  nodes: Map<string, Rect>;
  /** Each edge's right-angled path, by key. */
  routes: Map<string, Point[]>;
  /** Where edges sharing a way split or join. */
  joins: Point[];
}

/** An edge's way: its ends and, between groups, a port on each group it leaves or enters. */
interface Leg {
  from: string;
  to: string;
}

/** Splits each edge into legs. Edges between two groups leave by one port per far group and
 * come in by one port per near group, so they share one trunk between the groups, like a bus. */
export function legs(input: LayoutInput) {
  const groupOf = new Map<string, string>();
  for (const g of input.groups) for (const c of g.children) groupOf.set(c, g.id);
  const ports = new Map<string, { group: string; side: "EAST" | "WEST" }>();
  const legsOf = new Map<string, string[]>();
  const all = new Map<string, Leg>();
  for (const e of input.edges) {
    const from = groupOf.get(e.from);
    const to = groupOf.get(e.to);
    const stops = [e.from];
    if (from !== to) {
      if (from) {
        const id = `out:${from}>${to ?? e.to}`;
        ports.set(id, { group: from, side: "EAST" });
        stops.push(id);
      }
      if (to) {
        const id = `in:${to}<${from ?? e.from}`;
        ports.set(id, { group: to, side: "WEST" });
        stops.push(id);
      }
    }
    stops.push(e.to);
    const ids: string[] = [];
    for (let i = 1; i < stops.length; i++) {
      const id = `${stops[i - 1]}>>${stops[i]}`;
      all.set(id, { from: stops[i - 1], to: stops[i] });
      ids.push(id);
    }
    legsOf.set(e.key, ids);
  }
  return { ports, legsOf, all };
}

/** Lays the nodes out in their groups, with users left of what they use. */
export async function layoutArch(input: LayoutInput): Promise<Placed> {
  const { ports, legsOf, all } = legs(input);
  const grouped = new Map<string, ElkNode>();
  const root: ElkNode = {
    id: "root",
    layoutOptions: {
      "elk.algorithm": "layered",
      "elk.direction": "RIGHT",
      "elk.hierarchyHandling": "INCLUDE_CHILDREN",
      "elk.spacing.nodeNode": "28",
      "elk.layered.spacing.nodeNodeBetweenLayers": "60",
      "elk.layered.thoroughness": "2",
      "elk.padding": "[top=20,left=20,bottom=20,right=20]",
      "elk.edgeRouting": "ORTHOGONAL",
      "elk.layered.mergeEdges": "true",
      "elk.json.edgeCoords": "ROOT",
    },
    children: [],
    edges: [...all].map(([id, l]) => ({ id, sources: [l.from], targets: [l.to] })),
  };
  const groupOf = new Map<string, ElkNode>();
  for (const g of input.groups) {
    const node: ElkNode = {
      id: g.id,
      layoutOptions: {
        "elk.padding": `[top=${HEADER},left=18,bottom=16,right=18]`,
        "elk.portConstraints": "FIXED_SIDE",
      },
      children: [],
      ports: [...ports]
        .filter(([, p]) => p.group === g.id)
        .map(([id, p]) => ({
          id,
          width: PORT,
          height: PORT,
          layoutOptions: { "elk.port.side": p.side },
        })),
    };
    grouped.set(g.id, node);
    root.children?.push(node);
    for (const c of g.children) groupOf.set(c, node);
  }
  for (const id of input.nodes) {
    (groupOf.get(id) ?? root).children?.push({ id, width: NODE_W, height: NODE_H });
  }
  const out = await (await getElk()).layout(root);
  const groups: Placed["groups"] = new Map();
  const nodes: Placed["nodes"] = new Map();
  const rect = (n: ElkNode, ox: number, oy: number): Rect => ({
    x: ox + (n.x ?? 0),
    y: oy + (n.y ?? 0),
    width: n.width ?? 0,
    height: n.height ?? 0,
  });
  for (const c of out.children ?? []) {
    const at = rect(c, 0, 0);
    if (!grouped.has(c.id)) {
      nodes.set(c.id, at);
      continue;
    }
    groups.set(c.id, at);
    for (const n of c.children ?? []) nodes.set(n.id, rect(n, at.x, at.y));
  }
  const legPoints = new Map<string, Point[]>();
  const joins: Point[] = [];
  for (const e of out.edges ?? []) {
    const s = e.sections?.[0];
    if (s) legPoints.set(e.id, [s.startPoint, ...(s.bendPoints ?? []), s.endPoint]);
    joins.push(...(e.junctionPoints ?? []));
  }
  const routes: Placed["routes"] = new Map();
  for (const [key, ids] of legsOf) {
    const points = ids.flatMap((id) => legPoints.get(id) ?? []);
    if (points.length >= 2) routes.set(key, points);
  }
  return { groups, nodes, routes, joins };
}
