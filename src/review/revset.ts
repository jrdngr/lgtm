import type { WorkspaceName } from "../core/workspace";
import { prOf } from "../github/pr";
import { workspaceStore } from "./workspaceStore";

/** The revset each workspace reviews: the last one a review in it loaded. Review panes and
 * the idle review open on it, so it outlives any one pane. */
const store = workspaceStore<string>();

export const workspaceRevset = store.get;
export const setWorkspaceRevset = store.publish;

/** What a workspace's next review opens: its revset, else its PR, else the working copy. A
 * revset from a past run isn't reused, so one that no longer resolves can't break startup. */
export async function startRevset(workspace: WorkspaceName) {
  return store.get(workspace) ?? (await prOf(workspace))?.revset ?? "@";
}
