import type { ElkNode } from "elkjs/lib/elk-api.js";
import { getElk, type Point, type Rect } from "../graph/layout";

export const NODE_W = 150;
export const NODE_H = 48;
/** Room above a container's components for its header band. */
const HEADER = 34;

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
}

/** Lays the nodes out in their groups, with users left of what they use. */
export async function layoutArch(input: LayoutInput): Promise<Placed> {
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
      "elk.json.edgeCoords": "ROOT",
    },
    children: [],
    edges: input.edges.map((e) => ({ id: e.key, sources: [e.from], targets: [e.to] })),
  };
  const groupOf = new Map<string, ElkNode>();
  for (const g of input.groups) {
    const node: ElkNode = {
      id: g.id,
      layoutOptions: { "elk.padding": `[top=${HEADER},left=18,bottom=16,right=18]` },
      children: [],
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
  const routes: Placed["routes"] = new Map();
  for (const e of out.edges ?? []) {
    const s = e.sections?.[0];
    if (s) routes.set(e.id, [s.startPoint, ...(s.bendPoints ?? []), s.endPoint]);
  }
  return { groups, nodes, routes };
}
