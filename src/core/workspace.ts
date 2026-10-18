import { createContext, useContext } from "react";

/** A workspace by name. `null` is the repo's main workspace. */
export type WorkspaceName = string | null;

export const workspaceKey = (name: WorkspaceName) => name ?? "";

const roots = new Map<string, string>();

/** A workspace's directory, once the workspace list has loaded. */
export const workspaceRoot = (name: WorkspaceName) => roots.get(workspaceKey(name));

/** Records a workspace's directory as the workspace list loads. */
export const setWorkspaceRoot = (name: WorkspaceName, root: string) => {
  roots.set(workspaceKey(name), root);
};

/** The workspace whose dock a panel is in. */
export const WorkspaceContext = createContext<WorkspaceName>(null);

export function useWorkspace(): WorkspaceName {
  return useContext(WorkspaceContext);
}
