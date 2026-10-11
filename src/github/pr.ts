import { useEffect, useSyncExternalStore } from "react";
import type { PrDto } from "../bindings";
import { commands } from "../ipc";
import { type WorkspaceName, workspaceKey } from "../workspaces/context";

/** Each workspace's PR, `null` when it isn't a PR workspace. Absent until loaded. */
const prs = new Map<string, PrDto | null>();
const listeners = new Set<() => void>();
const loading = new Map<string, Promise<PrDto | null>>();

const subscribe = (l: () => void) => {
  listeners.add(l);
  return () => listeners.delete(l);
};

/** The workspace's PR, loading it the first time. */
export function prOf(workspace: WorkspaceName): Promise<PrDto | null> {
  const key = workspaceKey(workspace);
  if (prs.has(key)) return Promise.resolve(prs.get(key) ?? null);
  let p = loading.get(key);
  if (!p) {
    p = commands.prInfo(workspace).then((r) => {
      const pr = r.status === "ok" ? r.data : null;
      prs.set(key, pr);
      loading.delete(key);
      for (const l of listeners) l();
      return pr;
    });
    loading.set(key, p);
  }
  return p;
}

/** Loads the workspace's PR again, after a refetch changed it. */
export function reloadPr(workspace: WorkspaceName) {
  prs.delete(workspaceKey(workspace));
  return prOf(workspace);
}

/** The workspace's PR: `undefined` while loading, `null` when it has none. */
export function usePr(workspace: WorkspaceName): PrDto | null | undefined {
  useEffect(() => {
    prOf(workspace);
  }, [workspace]);
  return useSyncExternalStore(subscribe, () => prs.get(workspaceKey(workspace)));
}
