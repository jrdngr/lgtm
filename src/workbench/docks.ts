import type { DockviewApi } from "dockview-react";
import { type WorkspaceName, workspaceKey } from "../workspaces/context";

export type DockSlot = "main" | "bottom";

/** Each mounted workspace's docks, for opening panes from outside them. */
const docks: Record<DockSlot, Map<string, DockviewApi>> = { main: new Map(), bottom: new Map() };

export function registerDock(
  workspace: WorkspaceName,
  api: DockviewApi | undefined,
  slot: DockSlot = "main",
) {
  if (api) docks[slot].set(workspaceKey(workspace), api);
  else docks[slot].delete(workspaceKey(workspace));
}

/** The workspace's main dock, where panes open from outside it. */
export const dockOf = (workspace: WorkspaceName) => docks.main.get(workspaceKey(workspace));

export const bottomDockOf = (workspace: WorkspaceName) => docks.bottom.get(workspaceKey(workspace));

/** Whether the panel `id` is in one of the workspace's docks, or the main dock isn't up yet. */
export function inDock(workspace: WorkspaceName, id: string) {
  const api = dockOf(workspace);
  return !api || !!api.getPanel(id) || !!bottomDockOf(workspace)?.getPanel(id);
}
