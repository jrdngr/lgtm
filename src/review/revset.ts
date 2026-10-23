import type { View } from "../bindings";
import { type WorkspaceName, workspaceKey } from "../core/workspace";
import { prOf } from "../github/pr";
import { workspaceStore } from "./workspaceStore";

/** The revset each workspace reviews: the last one a review in it loaded. Review panes and
 * the idle review open on it, so it outlives any one pane. */
const store = workspaceStore<string>();

export const workspaceRevset = store.get;
export const setWorkspaceRevset = store.publish;
/** Drops a deleted workspace's revset, so a new workspace of its name starts fresh. */
export const forgetWorkspaceRevset = (workspace: WorkspaceName) =>
  store.retract(workspace, store.get(workspace));

/** What a workspace's next review opens: its revset, else its PR, else the working copy. A
 * revset from a past run isn't reused, so one that no longer resolves can't break startup. */
export async function startRevset(workspace: WorkspaceName) {
  return store.get(workspace) ?? (await prOf(workspace))?.revset ?? "@";
}

/** The view a workspace's next review pane opens in, once. */
const startViews = new Map<string, View>();

export function setStartView(workspace: WorkspaceName, view: View) {
  startViews.set(workspaceKey(workspace), view);
}

export function takeStartView(workspace: WorkspaceName): View | undefined {
  const view = startViews.get(workspaceKey(workspace));
  startViews.delete(workspaceKey(workspace));
  return view;
}
