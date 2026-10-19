import { useSyncExternalStore } from "react";
import { dockOf } from "../core/docks";
import { openArchitecture } from "../core/panels";
import { type WorkspaceName, workspaceKey } from "../core/workspace";

interface Show {
  workspace: string;
  id: string;
}
type Listener = (s: Show) => void;
const listeners = new Set<Listener>();
/** The last show, kept for an Architecture tab that mounts only once it's open. */
let pending: Show | undefined;

/** Shows node `id`, the edge `from>to` between two nodes, or starts tour `tour:<id>`, in the
 * workspace's Architecture tab, opening the tab. */
export function showInSystem(workspace: WorkspaceName, id: string) {
  pending = { workspace: workspaceKey(workspace), id };
  for (const l of listeners) l(pending);
  const api = dockOf(workspace);
  if (api) openArchitecture(api);
}

/** Calls `l` for each node shown in `workspace`, starting with one not yet taken. */
export function onShowInSystem(workspace: WorkspaceName, l: (id: string) => void): () => void {
  const key = workspaceKey(workspace);
  const mine: Listener = (s) => {
    if (s.workspace !== key) return;
    pending = undefined;
    l(s.id);
  };
  if (pending) mine(pending);
  listeners.add(mine);
  return () => {
    listeners.delete(mine);
  };
}

/** The node each workspace's Architecture tab has selected, for the sidebar to mark. */
const selected = new Map<string, string | undefined>();
const watchers = new Set<() => void>();

export function setSelectedNode(workspace: WorkspaceName, id: string | undefined) {
  const key = workspaceKey(workspace);
  if (selected.get(key) === id) return;
  selected.set(key, id);
  for (const w of watchers) w();
}

export function useSelectedNode(workspace: WorkspaceName): string | undefined {
  return useSyncExternalStore(
    (w) => {
      watchers.add(w);
      return () => watchers.delete(w);
    },
    () => selected.get(workspaceKey(workspace)),
  );
}
