import { useEffect, useRef, useState } from "react";
import type { WorkspaceDto } from "../bindings";
import { commands, errorText, unwrap } from "../ipc";
import { type WorkspaceName, workspaceKey } from "./context";
import { useRepoChanged } from "./useRepoChanged";

const roots = new Map<string, string>();

/** A workspace's directory, once the workspace list has loaded. */
export const workspaceRoot = (name: WorkspaceName) => roots.get(workspaceKey(name));

/** The repo's workspaces, kept current as the repo changes.
 * Reloads requested while one runs coalesce into one more, a second later. `error` is the
 * last action's failure, set with `setError` and kept across reloads, or else the last
 * reload's. */
export function useWorkspaces() {
  const [workspaces, setWorkspaces] = useState<WorkspaceDto[]>();
  const [loadError, setLoadError] = useState<string>();
  const [error, setError] = useState<string>();
  const busy = useRef(false);
  const again = useRef(false);

  async function reload() {
    if (busy.current) {
      again.current = true;
      return;
    }
    busy.current = true;
    try {
      const list = await unwrap(commands.listWorkspaces());
      setWorkspaces(list);
      for (const w of list) roots.set(workspaceKey(w.name), w.root);
      setLoadError(undefined);
    } catch (e) {
      setLoadError(errorText(e));
    } finally {
      busy.current = false;
    }
    if (again.current) {
      again.current = false;
      // An agent editing in a workspace makes changes back to back, so don't chase every one.
      window.setTimeout(() => reloadRef.current(), 1000);
    }
  }

  const reloadRef = useRef(reload);
  reloadRef.current = reload;

  useEffect(() => {
    reloadRef.current();
  }, []);
  useRepoChanged(() => reloadRef.current());

  return { workspaces, error: error ?? loadError, setError, reload };
}

export type Workspaces = ReturnType<typeof useWorkspaces>;
