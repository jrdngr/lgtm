import { dockOf } from "../core/docks";
import { openReviewTab } from "../core/panels";
import { requests } from "../core/request";
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

const focuses = requests<void>();

/** Gives keys to the revisions log. */
export const focusLog = () => focuses.send();
export const onFocusLog = focuses.listen;

const motions = requests<void>();

/** Opens the revisions log's motions. */
export const openMotions = () => motions.send();
export const onOpenMotions = motions.listen;
