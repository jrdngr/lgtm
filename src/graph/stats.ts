import type { GraphDto, GraphNode } from "../bindings";
import { openFile } from "../editor/open";
import type { CurrentReview } from "../review/current";
import type { WorkspaceName } from "../workspaces/context";
import { nodeRisk, rowsDone, showNode, worseRisk } from "./nodes";
import type { Box, Tree } from "./tree";

export interface BoxStats {
  /** Changed symbols in it. */
  count: number;
  /** Its changed rows that are done, and all of them. */
  done: number;
  all: number;
  risk?: string;
  /** Everything in it is an unchanged caller. */
  outside: boolean;
}

/** Totals for each box over the symbols in the graph that it holds. */
export function boxStats(
  review: CurrentReview,
  graph: GraphDto,
  tree: Tree,
  present: boolean[],
): Map<string, BoxStats> {
  const out = new Map<string, BoxStats>();
  for (const n of graph.nodes) {
    if (!present[n.id]) continue;
    const changed = n.hop === 0;
    const [done, all] = changed ? rowsDone(review, n) : [0, 0];
    const risk = changed ? nodeRisk(review, n) : undefined;
    for (
      let b = tree.boxes.get(tree.boxOf[n.id]);
      b;
      b = b.parent ? tree.boxes.get(b.parent) : undefined
    ) {
      const s = out.get(b.id) ?? { count: 0, done: 0, all: 0, outside: true };
      out.set(b.id, {
        count: s.count + (changed ? 1 : 0),
        done: s.done + done,
        all: s.all + all,
        risk: worseRisk(s.risk, risk),
        outside: s.outside && !changed,
      });
    }
  }
  return out;
}

/** The first symbol in a box that passes `keep`, in the box or deeper. */
function firstNode(
  graph: GraphDto,
  tree: Tree,
  box: Box,
  keep: (n: number) => boolean,
): GraphNode | undefined {
  const own = box.own.find(keep);
  if (own !== undefined) return graph.nodes[own];
  for (const c of box.children) {
    const child = tree.boxes.get(c);
    const n = child && firstNode(graph, tree, child, keep);
    if (n) return n;
  }
  return undefined;
}

/** Shows a box's code: a file from its top, or a symbol or group's first shown symbol,
 * changed ones first. */
export function jumpToBox(
  review: CurrentReview,
  graph: GraphDto,
  tree: Tree,
  box: Box,
  present: boolean[],
  workspace: WorkspaceName,
) {
  const n =
    firstNode(graph, tree, box, (i) => present[i] && graph.nodes[i].hop === 0) ??
    firstNode(graph, tree, box, (i) => present[i]);
  if (!n || box.kind === "folder") return;
  if (box.kind !== "file") return showNode(review, n, workspace);
  if (n.file === null) openFile(workspace, n.path);
  else review.jump(n.file);
}
