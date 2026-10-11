import type { ELK as Elk, ElkNode } from "elkjs/lib/elk-api.js";
import type { GraphDto, GraphNode } from "../bindings";
import { nodeLabel } from "./nodes";

let elk: Promise<Elk> | undefined;

/** ELK in a web worker, so a big graph doesn't block the window. Tests have no workers and
 * run it inline. */
function getElk(): Promise<Elk> {
  elk ??=
    typeof Worker === "undefined"
      ? import("elkjs/lib/elk.bundled.js").then((m) => new m.default())
      : Promise.all([
          import("elkjs/lib/elk-api.js"),
          import("elkjs/lib/elk-worker.min.js?worker"),
        ]).then(([api, worker]) => new api.default({ workerFactory: () => new worker.default() }));
  return elk;
}

export const NODE_HEIGHT = 40;
const nodeWidth = (n: GraphNode) => Math.max(90, nodeLabel(n).length * 7 + 36);

export interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface Placed {
  /** File groups by path, positioned in the graph. */
  groups: Map<string, Box>;
  /** Symbol nodes by id, positioned inside their file's group. */
  nodes: Map<number, Box>;
}

/** Lays out nodes in boxes per file, callers to the left of what they call. */
export async function layout(graph: GraphDto): Promise<Placed> {
  const byPath = new Map<string, GraphNode[]>();
  for (const n of graph.nodes) byPath.set(n.path, [...(byPath.get(n.path) ?? []), n]);
  const root: ElkNode = {
    id: "root",
    layoutOptions: {
      "elk.algorithm": "layered",
      "elk.direction": "RIGHT",
      "elk.hierarchyHandling": "INCLUDE_CHILDREN",
      "elk.spacing.nodeNode": "16",
      "elk.layered.spacing.nodeNodeBetweenLayers": "48",
      "elk.layered.thoroughness": "2",
      "elk.padding": "[top=12,left=12,bottom=12,right=12]",
    },
    children: [...byPath].map(([path, nodes]) => ({
      id: `file:${path}`,
      layoutOptions: { "elk.padding": "[top=30,left=10,bottom=10,right=10]" },
      children: nodes.map((n) => ({ id: String(n.id), width: nodeWidth(n), height: NODE_HEIGHT })),
    })),
    edges: graph.edges.map((e, i) => ({
      id: `e${i}`,
      sources: [String(e.from)],
      targets: [String(e.to)],
    })),
  };
  const out = await (await getElk()).layout(root);
  const groups = new Map<string, Box>();
  const nodes = new Map<number, Box>();
  const box = (n: ElkNode): Box => ({
    x: n.x ?? 0,
    y: n.y ?? 0,
    width: n.width ?? 0,
    height: n.height ?? 0,
  });
  for (const g of out.children ?? []) {
    groups.set(g.id.slice("file:".length), box(g));
    for (const c of g.children ?? []) nodes.set(Number(c.id), box(c));
  }
  return { groups, nodes };
}
