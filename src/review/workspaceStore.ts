import { useSyncExternalStore } from "react";
import { type WorkspaceName, workspaceKey } from "../workspaces/context";

/** One value per workspace, published by the pane that owns it, for views that follow it. */
export function workspaceStore<T>() {
  const items = new Map<string, T>();
  const listeners = new Set<() => void>();
  const changed = () => {
    for (const l of listeners) l();
  };
  const subscribe = (l: () => void) => {
    listeners.add(l);
    return () => listeners.delete(l);
  };
  const get = (workspace: WorkspaceName) => items.get(workspaceKey(workspace));
  return {
    get,
    publish(workspace: WorkspaceName, next: T) {
      items.set(workspaceKey(workspace), next);
      changed();
    },
    /** Clears the workspace's value only if `owner` is still it. */
    retract(workspace: WorkspaceName, owner: T | undefined) {
      if (owner === undefined || get(workspace) !== owner) return;
      items.delete(workspaceKey(workspace));
      changed();
    },
    isOwner: (workspace: WorkspaceName, owner: T | undefined) =>
      owner !== undefined && get(workspace) === owner,
    use: (workspace: WorkspaceName) => useSyncExternalStore(subscribe, () => get(workspace)),
  };
}
