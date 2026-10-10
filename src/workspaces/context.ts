import { createContext, useContext } from "react";

/** A workspace by name. `null` is the repo's main workspace. */
export type WorkspaceName = string | null;

export const workspaceKey = (name: WorkspaceName) => name ?? "";

/** The workspace whose dock a panel is in. */
export const WorkspaceContext = createContext<WorkspaceName>(null);

export function useWorkspace(): WorkspaceName {
  return useContext(WorkspaceContext);
}
