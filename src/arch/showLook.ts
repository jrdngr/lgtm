import { type WorkspaceName, workspaceKey } from "../core/workspace";

type Listener = (workspace: string) => void;
const listeners = new Set<Listener>();
/** The last ask, kept for a Look list that mounts only once its tab is shown. */
let pending: string | undefined;

/** Shows the workspace's Look list in the sidebar and flashes its open items. */
export function showLook(workspace: WorkspaceName) {
  pending = workspaceKey(workspace);
  for (const l of listeners) l(pending);
}

/** Calls `l` for each ask in `workspace`, starting with one not yet taken. */
export function onShowLook(workspace: WorkspaceName, l: () => void): () => void {
  const key = workspaceKey(workspace);
  const mine: Listener = (w) => {
    if (w !== key) return;
    pending = undefined;
    l();
  };
  if (pending) mine(pending);
  listeners.add(mine);
  return () => listeners.delete(mine);
}

/** Calls `l` for every ask, whichever workspace it's in. */
export function onAnyShowLook(l: () => void): () => void {
  const any: Listener = () => l();
  listeners.add(any);
  return () => listeners.delete(any);
}
