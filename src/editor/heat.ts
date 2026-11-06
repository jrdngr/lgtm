import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect, useSyncExternalStore } from "react";
import { events } from "../bindings";
import { remembered } from "../core/remembered";
import { type WorkspaceName, workspaceKey } from "../core/workspace";

/** How long a write glows in the file trees. */
export const HEAT_MS = 3000;

/** Whether the file trees show heat. */
export const heatOn = remembered("fileHeat", true, [true, false]);

/** When each path of a workspace, and each folder above it, was last written. */
export interface Heat {
  files: Map<string, number>;
  dirs: Map<string, number>;
}

const EMPTY: Heat = { files: new Map(), dirs: new Map() };
const heat = new Map<string, Heat>();
const listeners = new Set<() => void>();
let listening = false;

function written(workspace: string | null, paths: string[]) {
  const key = workspaceKey(workspace);
  const now = Date.now();
  const old = heat.get(key) ?? EMPTY;
  const fresh = (m: Map<string, number>) => new Map([...m].filter(([, at]) => now - at < HEAT_MS));
  const next = { files: fresh(old.files), dirs: fresh(old.dirs) };
  for (const p of paths) {
    next.files.set(p, now);
    for (let i = p.lastIndexOf("/"); i > 0; i = p.lastIndexOf("/", i - 1)) {
      next.dirs.set(p.slice(0, i), now);
    }
  }
  heat.set(key, next);
  for (const l of listeners) l();
}

function listen() {
  if (listening) return;
  listening = true;
  events.filesWritten(getCurrentWebviewWindow()).listen((e) => {
    written(e.payload.workspace, e.payload.paths);
  });
}

const subscribe = (l: () => void) => {
  listeners.add(l);
  return () => listeners.delete(l);
};

/** The workspace's recent writes, while heat is on. */
export function useHeat(workspace: WorkspaceName): Heat {
  useEffect(listen, []);
  const on = heatOn.use();
  const h = useSyncExternalStore(subscribe, () => heat.get(workspaceKey(workspace)) ?? EMPTY);
  return on ? h : EMPTY;
}
