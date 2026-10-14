import { useEffect } from "react";
import { dockOf, hasPanel, revealPanel } from "../workbench/docks";
import { openThreads } from "../workbench/panels";
import { type WorkspaceName, workspaceKey } from "../workspaces/context";

type Listener = (id: number) => void;
const listeners = new Map<string, Set<Listener>>();
/** A focus asked for before the tab mounted, taken when it does. */
const waiting = new Map<string, number>();

/** Whether the workspace has a Threads tab open. */
export const threadsOpen = (workspace: WorkspaceName) => hasPanel(workspace, "threads");

/** Opens the workspace's Threads tab, or brings it forward, with thread `id` focused. */
export function showThread(workspace: WorkspaceName, id?: number) {
  const api = dockOf(workspace);
  if (!api) return;
  const key = workspaceKey(workspace);
  if (id !== undefined) {
    const ls = listeners.get(key);
    if (ls?.size) for (const l of ls) l(id);
    else waiting.set(key, id);
  }
  if (!revealPanel(workspace, "threads")) openThreads(api);
}

/** Calls `onFocus` when something asks the Threads tab to focus a thread. */
export function useFocusRequests(workspace: WorkspaceName, onFocus: Listener) {
  useEffect(() => {
    const key = workspaceKey(workspace);
    const ls = listeners.get(key) ?? new Set();
    listeners.set(key, ls);
    ls.add(onFocus);
    const early = waiting.get(key);
    if (early !== undefined) {
      waiting.delete(key);
      onFocus(early);
    }
    return () => {
      ls.delete(onFocus);
    };
  }, [workspace, onFocus]);
}
