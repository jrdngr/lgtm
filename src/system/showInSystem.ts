import { dockOf } from "../core/docks";
import { openSystem } from "../core/panels";
import { type WorkspaceName, workspaceKey } from "../core/workspace";

interface Show {
  workspace: string;
  id: string;
}
type Listener = (s: Show) => void;
const listeners = new Set<Listener>();
/** The last show, kept for a System tab that mounts only once it's open. */
let pending: Show | undefined;

/** Shows node `id` in the workspace's System tab, opening the tab. */
export function showInSystem(workspace: WorkspaceName, id: string) {
  pending = { workspace: workspaceKey(workspace), id };
  for (const l of listeners) l(pending);
  const api = dockOf(workspace);
  if (api) openSystem(api);
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
