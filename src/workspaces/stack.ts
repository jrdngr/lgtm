import type { PrDto } from "../bindings";
import { setPr } from "../github/pr";
import { commands, unwrap } from "../ipc";
import { setWorkspaceRevset } from "../review/revset";
import { targetOf } from "../review/target";
import type { WorkspaceName } from "./context";

/** Points the workspace's review at `revset`, in its review pane if one is open. */
function review(workspace: WorkspaceName, revset: string) {
  setWorkspaceRevset(workspace, revset);
  targetOf(workspace)?.load(revset);
}

/** Shows `next` as the workspace's PR. A new stack PR opens the whole PR in the review, and
 * losing one goes back to `@`. */
export function showStackPr(
  workspace: WorkspaceName,
  before: PrDto | null | undefined,
  next: PrDto | null,
) {
  setPr(workspace, next);
  if (next?.stack && next.number !== before?.number) review(workspace, next.revset);
  else if (!next && before?.stack) review(workspace, "@");
}

/** Deselects the workspace's stack PR. */
export async function clearStackPr(workspace: WorkspaceName, before: PrDto) {
  await unwrap(commands.clearStackPr(workspace));
  showStackPr(workspace, before, null);
}
