import type { ELK as Elk, ElkNode } from "elkjs/lib/elk-api.js";
import type { Routed } from "./ports";
import type { Tree } from "./tree";

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
/** Room for an open box's header above what's in it. */
const HEADER = 30;
/** Room for a callers badge. */
const BADGE = 80;
const labelWidth = (label: string, badge: boolean) =>
  Math.max(90, label.length * 7 + 56 + (badge ? BADGE : 0));

export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface Placed {
  /** Boxes by id: `at` is in the parent box's coordinates, `abs` in the graph's. */
  boxes: Map<string, { at: Rect; abs: Rect; open: boolean }>;
  /** Port centers by id, in the graph's coordinates. */
  ports: Map<string, { x: number; y: number }>;
}

const PORT = 8;

/** What to lay out: the boxes on screen, parents first, which of them are open, and the
 * links between them, routed through the open boxes' ports. */
export interface Scene {
  tree: Tree;
  visible: string[];
  open: ReadonlySet<string>;
  routed: Routed;
  /** Boxes with a callers badge. */
  badges: ReadonlySet<string>;
}

/** Lays out the boxes, nested, with users to the left of what they use. */
export async function layout(scene: Scene): Promise<Placed> {
  const { tree, visible, open } = scene;
  const elkNodes = new Map<string, ElkNode>();
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
    children: [],
    edges: [...scene.routed.segments.values()].map((l, i) => ({
      id: `e${i}`,
      sources: [l.from],
      targets: [l.to],
    })),
  };
  const portsOf = new Map<string, ElkNode["ports"]>();
  for (const p of scene.routed.ports.values()) {
    const side = p.side === "out" ? "EAST" : "WEST";
    const list = portsOf.get(p.box) ?? [];
    list.push({ id: p.id, width: PORT, height: PORT, layoutOptions: { "elk.port.side": side } });
    portsOf.set(p.box, list);
  }
  for (const id of visible) {
    const box = tree.boxes.get(id);
    if (!box) continue;
    const width = labelWidth(box.label, scene.badges.has(id));
    const node: ElkNode = open.has(id)
      ? {
          id,
          layoutOptions: {
            "elk.padding": `[top=${HEADER},left=10,bottom=10,right=10]`,
            "elk.nodeSize.constraints": "MINIMUM_SIZE",
            "elk.nodeSize.minimum": `(${width}, ${HEADER})`,
            "elk.portConstraints": "FIXED_SIDE",
          },
          children: [],
          ports: portsOf.get(id) ?? [],
        }
      : { id, width, height: NODE_HEIGHT };
    elkNodes.set(id, node);
    const parent = (box.parent && elkNodes.get(box.parent)) || root;
    parent.children?.push(node);
  }
  const out = await (await getElk()).layout(root);
  const boxes: Placed["boxes"] = new Map();
  const ports: Placed["ports"] = new Map();
  const walk = (n: ElkNode, ox: number, oy: number) => {
    for (const c of n.children ?? []) {
      const at = { x: c.x ?? 0, y: c.y ?? 0, width: c.width ?? 0, height: c.height ?? 0 };
      const abs = { ...at, x: ox + at.x, y: oy + at.y };
      boxes.set(c.id, { at, abs, open: open.has(c.id) });
      for (const p of c.ports ?? []) {
        ports.set(p.id, { x: abs.x + (p.x ?? 0) + PORT / 2, y: abs.y + (p.y ?? 0) + PORT / 2 });
      }
      walk(c, abs.x, abs.y);
    }
  };
  walk(out, 0, 0);
  return { boxes, ports };
}
