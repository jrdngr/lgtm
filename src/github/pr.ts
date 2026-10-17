import { useEffect, useSyncExternalStore } from "react";
import type { PrDto } from "../bindings";
import { commands } from "../ipc";
import { type WorkspaceName, workspaceKey } from "../workspaces/context";

/** Each workspace's PR, `null` when it has none. Absent until loaded. */
const prs = new Map<string, PrDto | null>();
const listeners = new Set<() => void>();
const loading = new Map<string, Promise<PrDto | null>>();

const subscribe = (l: () => void) => {
  listeners.add(l);
  return () => listeners.delete(l);
};

/** The workspace's PR, loading it the first time. A failed load is tried again `retries`
 * more times, a second apart. */
export function prOf(workspace: WorkspaceName, retries = 5): Promise<PrDto | null> {
  const key = workspaceKey(workspace);
  if (prs.has(key)) return Promise.resolve(prs.get(key) ?? null);
  let p = loading.get(key);
  if (!p) {
    p = commands.prInfo(workspace).then((r) => {
      loading.delete(key);
      // A failure, as before the window's repo is set, isn't remembered as no PR.
      if (r.status === "error") {
        if (retries > 0) window.setTimeout(() => prOf(workspace, retries - 1), 1000);
        return null;
      }
      prs.set(key, r.data);
      for (const l of listeners) l();
      return r.data;
    });
    loading.set(key, p);
  }
  return p;
}

/** Sets the workspace's PR, as picking one from its stack does. The same PR again changes
 * nothing, so what shows it doesn't reload. */
export function setPr(workspace: WorkspaceName, pr: PrDto | null) {
  const key = workspaceKey(workspace);
  if (prs.has(key) && JSON.stringify(prs.get(key)) === JSON.stringify(pr)) return;
  prs.set(key, pr);
  for (const l of listeners) l();
}

/** Loads the workspace's PR again, after a refetch changed it. The old one shows until then. */
export async function reloadPr(workspace: WorkspaceName): Promise<PrDto | null> {
  if (!prs.has(workspaceKey(workspace))) return prOf(workspace);
  const r = await commands.prInfo(workspace);
  if (r.status === "error") return prs.get(workspaceKey(workspace)) ?? null;
  setPr(workspace, r.data);
  return r.data;
}

/** The workspace's PR: `undefined` while loading, `null` when it has none. */
export function usePr(workspace: WorkspaceName): PrDto | null | undefined {
  useEffect(() => {
    prOf(workspace);
  }, [workspace]);
  return useSyncExternalStore(subscribe, () => prs.get(workspaceKey(workspace)));
}
