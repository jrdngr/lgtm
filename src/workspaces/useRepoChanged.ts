import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect, useRef } from "react";
import { events } from "../bindings";
import { coalesce } from "./coalesce";

const listeners = new Set<() => void>();
let listening = false;

/** Calls `cb` when this window's repo changes. Every workspace's watcher reports each jj
 * operation, and an agent's snapshots land every few seconds, so a run of reports reloads
 * once. Returns a function that stops it. */
export function onRepoChanged(cb: () => void): () => void {
  if (!listening) {
    listening = true;
    const fire = coalesce(
      () => {
        for (const l of listeners) l();
      },
      250,
      1000,
    );
    // Scoped to this window: other windows hold other repos.
    events.repoChanged(getCurrentWebviewWindow()).listen(fire);
  }
  listeners.add(cb);
  return () => {
    listeners.delete(cb);
  };
}

/** Calls the latest `cb` whenever this window's repo changes. */
export function useRepoChanged(cb: () => void) {
  const ref = useRef(cb);
  ref.current = cb;
  useEffect(() => onRepoChanged(() => ref.current()), []);
}
