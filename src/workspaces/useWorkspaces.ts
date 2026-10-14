import { useEffect, useRef, useState } from "react";
import type { ProgressDto, WorkspaceDto } from "../bindings";
import { commands, errorText, unwrap } from "../ipc";
import { type WorkspaceName, workspaceKey } from "./context";
import { useRepoChanged } from "./useRepoChanged";

const listeners = new Set<() => void>();
const roots = new Map<string, string>();

/** A workspace's directory, once the workspace list has loaded. */
export const workspaceRoot = (name: WorkspaceName) => roots.get(workspaceKey(name));

/** Tells the workspace list that marks or a workspace's review revset changed, which no jj
 * operation reports. */
export function progressChanged() {
  for (const l of listeners) l();
}

/** The repo's workspaces and each one's review progress, kept current as the repo changes.
 * Reloads requested while one runs coalesce into one more, a second later. `error` is the
 * last action's failure, set with `setError` and kept across reloads, or else the last
 * reload's. */
export function useWorkspaces() {
  const [workspaces, setWorkspaces] = useState<WorkspaceDto[]>();
  const [progress, setProgress] = useState<ReadonlyMap<string, ProgressDto>>(new Map());
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
      const entries = await Promise.all(
        list.map(async (w) => {
          const r = await commands.workspaceProgress(w.name);
          return r.status === "ok" ? ([workspaceKey(w.name), r.data] as const) : undefined;
        }),
      );
      setProgress(new Map(entries.filter((e) => e !== undefined)));
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
    const onChange = () => reloadRef.current();
    listeners.add(onChange);
    return () => {
      listeners.delete(onChange);
    };
  }, []);
  useRepoChanged(() => reloadRef.current());

  return { workspaces, progress, error: error ?? loadError, setError, reload };
}

export type Workspaces = ReturnType<typeof useWorkspaces>;
