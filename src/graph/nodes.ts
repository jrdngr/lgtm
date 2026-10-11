import type { GraphNode } from "../bindings";
import { openFile } from "../editor/open";
import type { CurrentReview } from "../review/current";
import type { WorkspaceName } from "../workspaces/context";

/** A node's label: `Parent::name` for a member. */
export const nodeLabel = (n: GraphNode) => (n.parent ? `${n.parent}::${n.name}` : n.name);

/** Share of a changed node's rows that are reviewed or skipped, or undefined for callers. */
export function nodeDone(review: CurrentReview, n: GraphNode): number | undefined {
  if (n.file === null || n.rows.length === 0) return undefined;
  const file = n.file;
  const done = n.rows.filter((r) => review.rowState(file, r) !== "unreviewed").length;
  return done / n.rows.length;
}

/** Shows a node's code in the review: its first changed row, or the file for a caller in
 * one of the review's files. Callers elsewhere open in the editor. */
export function showNode(review: CurrentReview, n: GraphNode, workspace: WorkspaceName) {
  if (n.file === null) return openFile(workspace, n.path, n.line);
  if (n.rows.length > 0) review.jumpToRow(n.file, n.rows[0]);
  else review.jump(n.file);
}

export const STATUS_SIGN = { added: "+", modified: "~", removed: "−", caller: "" } as const;

/** The worst hotspot level on a node's rows, if any. Hotspots come worst first. */
export function nodeRisk(review: CurrentReview, n: GraphNode): string | undefined {
  const hit = review.hotspots.find(
    (h) =>
      h.rows !== null &&
      h.file === n.file &&
      n.rows.some((r) => h.rows !== null && h.rows.start <= r && r <= h.rows.end),
  );
  return hit?.level;
}
