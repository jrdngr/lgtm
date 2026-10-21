import { dockOf } from "../core/docks";
import { openReviewTab } from "../core/panels";
import type { WorkspaceName } from "../core/workspace";
import { setWorkspaceRevset } from "../review/revset";
import { targetOf } from "../review/target";

/** Loads `revset` as the workspace's review. With no review pane, a review tab opens on it.
 * Returns the error message when it fails. */
export async function loadReview(
  workspace: WorkspaceName,
  revset: string,
): Promise<string | undefined> {
  const target = targetOf(workspace);
  if (target) return target.load(revset);
  setWorkspaceRevset(workspace, revset);
  const api = dockOf(workspace);
  if (api) openReviewTab(api);
  return undefined;
}

const focusers = new Set<() => void>();

/** Gives keys to the revisions log. */
export function focusLog() {
  for (const f of focusers) f();
}

export function onFocusLog(f: () => void) {
  focusers.add(f);
  return () => {
    focusers.delete(f);
  };
}

const motionOpeners = new Set<() => void>();

/** Opens the revisions log's motions. */
export function openMotions() {
  for (const f of motionOpeners) f();
}

export function onOpenMotions(f: () => void) {
  motionOpeners.add(f);
  return () => {
    motionOpeners.delete(f);
  };
}
