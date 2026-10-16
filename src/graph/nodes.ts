import type { GraphNode } from "../bindings";
import { openFile } from "../editor/open";
import type { CurrentReview } from "../review/current";
import { needsReview } from "../review/model";
import { stateDone } from "../review/state";
import type { WorkspaceName } from "../workspaces/context";

/** How many of a node's changed rows are done, and how many it has. */
export function rowsDone(review: CurrentReview, n: GraphNode): [number, number] {
  const file = n.file;
  if (file === null) return [0, 0];
  const rows = n.rows.filter((r) => needsReview(review.files[file].rows[r]));
  const done = rows.filter((r) => stateDone(review.rowState(file, r), review.done)).length;
  return [done, rows.length];
}

/** Shows a node's code in the review: its first changed row, or the file for a caller in
 * one of the review's files. Callers elsewhere open in the editor. */
export function showNode(review: CurrentReview, n: GraphNode, workspace: WorkspaceName) {
  if (n.file === null) return openFile(workspace, n.path, n.line);
  if (n.rows.length > 0) review.jumpToRow(n.file, n.rows[0]);
  else review.jump(n.file);
}

export const STATUS_SIGN = { added: "+", modified: "~", removed: "−", caller: "" } as const;

const RISK_ORDER = ["high", "medium", "low"];

/** The worse of two note levels. */
export const worseRisk = (a?: string, b?: string) =>
  a === undefined || (b !== undefined && RISK_ORDER.indexOf(b) < RISK_ORDER.indexOf(a)) ? b : a;

/** The worst level of the important notes on a node's rows, if any. */
export function nodeRisk(review: CurrentReview, n: GraphNode): string | undefined {
  let worst: string | undefined;
  for (const note of review.notes) {
    const rows = note.rows;
    if (note.category !== "important" || !note.level || !rows || note.file !== n.file) continue;
    if (n.rows.some((r) => rows.start <= r && r <= rows.end)) worst = worseRisk(worst, note.level);
  }
  return worst;
}
