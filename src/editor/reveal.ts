import { type WorkspaceName, workspaceKey } from "../core/workspace";

interface Reveal {
  workspace: WorkspaceName;
  path: string;
}
type Listener = (r: Reveal) => void;
const listeners = new Set<Listener>();
/** The last reveal, kept for a Files tab that mounts only once it's shown. */
let pending: Reveal | undefined;

/** Shows `path` in the Files tab: the Workbench opens the tab and the tree opens to it. */
export function revealInFiles(workspace: WorkspaceName, path: string) {
  pending = { workspace, path };
  for (const l of listeners) l(pending);
}

/** Calls `l` for each reveal in `workspace`, starting with one not yet taken. */
export function onRevealInFiles(workspace: WorkspaceName, l: (path: string) => void): () => void {
  const mine: Listener = (r) => {
    if (workspaceKey(r.workspace) !== workspaceKey(workspace)) return;
    pending = undefined;
    l(r.path);
  };
  if (pending) mine(pending);
  listeners.add(mine);
  return () => listeners.delete(mine);
}

/** Calls `l` for every reveal, whichever workspace it's in. */
export function onAnyReveal(l: () => void): () => void {
  const any: Listener = () => l();
  listeners.add(any);
  return () => listeners.delete(any);
}
