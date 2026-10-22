import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect, useRef } from "react";
import { events } from "../bindings";
import { type WorkspaceName, workspaceKey } from "../core/workspace";
import { coalesce } from "./coalesce";

interface Listener {
  cb: () => void;
  /** Set for a workspace's own views, which wait while the workspace is hidden. */
  key?: string;
}

const listeners = new Set<Listener>();
/** Listeners whose workspace was hidden when the repo changed, run once it shows. */
const missed = new Set<Listener>();
let shown: string | undefined;
let listening = false;

/** The workspace on screen. Its views reload on repo changes; the others catch up when
 * they're shown. */
export function showWorkspace(ws: WorkspaceName) {
  shown = workspaceKey(ws);
  for (const l of missed) {
    if (l.key !== shown) continue;
    missed.delete(l);
    l.cb();
  }
}

function fire() {
  for (const l of listeners) {
    if (l.key === undefined || l.key === shown) l.cb();
    else missed.add(l);
  }
}

/** Calls `cb` when this window's repo changes. Every workspace's watcher reports each jj
 * operation, and an agent's snapshots land every few seconds, so a run of reports reloads
 * once. With `workspace`, a change while it's hidden calls `cb` when it's next shown
 * instead. Returns a function that stops it. */
export function onRepoChanged(cb: () => void, workspace?: WorkspaceName): () => void {
  if (!listening) {
    listening = true;
    // Scoped to this window: other windows hold other repos.
    events.repoChanged(getCurrentWebviewWindow()).listen(coalesce(fire, 250, 1000));
  }
  const l: Listener = { cb, key: workspace === undefined ? undefined : workspaceKey(workspace) };
  listeners.add(l);
  return () => {
    listeners.delete(l);
    missed.delete(l);
  };
}

/** Calls the latest `cb` whenever this window's repo changes, as `onRepoChanged` does. */
export function useRepoChanged(cb: () => void, workspace?: WorkspaceName) {
  const ref = useRef(cb);
  ref.current = cb;
  const key = workspace === undefined ? undefined : workspaceKey(workspace);
  // biome-ignore lint/correctness/useExhaustiveDependencies: `key` stands for `workspace`
  useEffect(() => onRepoChanged(() => ref.current(), workspace), [key]);
}
