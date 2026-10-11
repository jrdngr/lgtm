import type { DockviewApi } from "dockview-react";
import { type WorkspaceName, workspaceKey } from "../workspaces/context";

/** Each mounted workspace's dock, for opening panes from outside it. */
const docks = new Map<string, DockviewApi>();

export function registerDock(workspace: WorkspaceName, api: DockviewApi | undefined) {
  if (api) docks.set(workspaceKey(workspace), api);
  else docks.delete(workspaceKey(workspace));
}

export const dockOf = (workspace: WorkspaceName) => docks.get(workspaceKey(workspace));
