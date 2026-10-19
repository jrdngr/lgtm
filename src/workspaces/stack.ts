import type { PrDto } from "../bindings";
import { loadState, saveState } from "../core/savedState";
import type { WorkspaceName } from "../core/workspace";
import { setPr } from "../github/pr";
import { commands, unwrap } from "../ipc";
import { setWorkspaceRevset } from "../review/revset";
import { targetOf } from "../review/target";

const SHOW_PRS = "stack-prs";
const isBool = (v: unknown): v is boolean => typeof v === "boolean";

/** Whether the Workspaces panel shows stack PRs. Off until the user turns it on; while it's
 * off, no stack PR gets picked. */
export const stackPrsShown = () => loadState(SHOW_PRS, false, isBool);

export const setStackPrsShown = (show: boolean) => saveState(SHOW_PRS, show);

/** Points the workspace's review at `revset`, in its review pane if one is open. */
function review(workspace: WorkspaceName, revset: string) {
  setWorkspaceRevset(workspace, revset);
  targetOf(workspace)?.load(revset);
}

/** Shows `next` as the workspace's PR. A new stack PR, or one `@` left or came back to, opens
 * its view in the review, and losing one goes back to `@`. */
export function showStackPr(
  workspace: WorkspaceName,
  before: PrDto | null | undefined,
  next: PrDto | null,
) {
  setPr(workspace, next);
  if (next?.stack && (next.number !== before?.number || next.off !== before?.off)) {
    review(workspace, next.revset);
  } else if (!next && before?.stack) review(workspace, "@");
}

/** Edits the change of the workspace's stack PR, which `@` left. */
export async function editBackToStackPr(workspace: WorkspaceName, before: PrDto) {
  showStackPr(workspace, before, await unwrap(commands.editBackToStackPr(workspace)));
}

/** Deselects the workspace's stack PR. */
export async function clearStackPr(workspace: WorkspaceName, before: PrDto) {
  await unwrap(commands.clearStackPr(workspace));
  showStackPr(workspace, before, null);
}
