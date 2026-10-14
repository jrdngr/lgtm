import { useSyncExternalStore } from "react";
import { type WorkspaceName, workspaceKey } from "./context";

export type ActivityState = "running" | "done" | "failed" | "stopped";

/** What a workspace's agents are up to, for its row in the sidebar. A run that ended while the
 * workspace wasn't selected stays finished until it's selected, and wins over running ones. */
export type Activity =
  | { kind: "working"; labels: string[] }
  | { kind: "finished"; failed: boolean; labels: string[] };

interface Finished {
  failed: boolean;
  labels: string[];
}

/** Running agents by id, with their workspace and label. */
const running = new Map<string, { key: string; label: string }>();
const finished = new Map<string, Finished>();
const cache = new Map<string, Activity | undefined>();
const listeners = new Set<() => void>();
let selected = "";

function changed(key: string) {
  cache.delete(key);
  for (const l of listeners) l();
}

/** Reports agent `id` in `workspace`. Only an agent reported running can end, so repeated
 * reports of how one ended are ignored. An ending the user didn't ask for counts as finished
 * unless its workspace is selected. */
export function report(workspace: WorkspaceName, id: string, state: ActivityState, label: string) {
  const key = workspaceKey(workspace);
  if (state === "running") {
    if (running.get(id)?.key === key) return;
    running.set(id, { key, label });
    return changed(key);
  }
  if (!running.delete(id)) return;
  if (state !== "stopped" && key !== selected) {
    const f = finished.get(key) ?? { failed: false, labels: [] };
    finished.set(key, {
      failed: f.failed || state === "failed",
      labels: f.labels.includes(label) ? f.labels : [...f.labels, label],
    });
  }
  changed(key);
}

/** Marks `workspace` as the one shown, which clears what finished in it. */
export function selectActivity(workspace: WorkspaceName) {
  selected = workspaceKey(workspace);
  clearFinished(workspace);
}

/** Clears the dot of what finished in `workspace`, without selecting it. */
export function clearFinished(workspace: WorkspaceName) {
  const key = workspaceKey(workspace);
  if (finished.delete(key)) changed(key);
}

export function activity(workspace: WorkspaceName): Activity | undefined {
  const key = workspaceKey(workspace);
  if (cache.has(key)) return cache.get(key);
  const done = finished.get(key);
  const labels = [...running.values()].filter((r) => r.key === key).map((r) => r.label);
  const a: Activity | undefined = done
    ? { kind: "finished", ...done }
    : labels.length
      ? { kind: "working", labels }
      : undefined;
  cache.set(key, a);
  return a;
}

function subscribe(l: () => void) {
  listeners.add(l);
  return () => listeners.delete(l);
}

export function useActivity(workspace: WorkspaceName): Activity | undefined {
  return useSyncExternalStore(subscribe, () => activity(workspace));
}
