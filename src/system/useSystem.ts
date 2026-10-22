import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useCallback, useEffect, useState, useSyncExternalStore } from "react";
import { events, type SystemDto } from "../bindings";
import { type WorkspaceName, workspaceKey } from "../core/workspace";
import { commands, errorText, unwrap } from "../ipc";
import { useViewSaves } from "./archView";

/** Bumped per workspace when its map needs loading again. */
const versions = new Map<string, number>();
const listeners = new Set<() => void>();
/** Each workspace's load in flight, shared by the panels that ask for its map meanwhile. */
const loads = new Map<string, { at: string; p: Promise<SystemDto> }>();
let listening = false;

function bump(key: string) {
  versions.set(key, (versions.get(key) ?? 0) + 1);
  for (const l of listeners) l();
}

function subscribe(l: () => void) {
  if (!listening) {
    listening = true;
    events.systemChanged(getCurrentWebviewWindow()).listen(({ payload: p }) => {
      bump(workspaceKey(p.workspace));
    });
  }
  listeners.add(l);
  return () => listeners.delete(l);
}

function load(workspace: WorkspaceName, version: number, saves: number) {
  const key = workspaceKey(workspace);
  const at = `${version}:${saves}`;
  let l = loads.get(key);
  if (!l || l.at !== at) {
    const p = unwrap(commands.systemMap(workspace));
    const made = { at, p };
    l = made;
    loads.set(key, made);
    const done = () => loads.get(key) === made && loads.delete(key);
    p.then(done, done);
  }
  return l.p;
}

/** The workspace's system map, loaded again when a terminal agent describes it, the view
 * changes, or on `reload`. Panels showing one workspace share a load.
 * Keeps the last map while the next loads. */
export function useSystem(workspace: WorkspaceName) {
  const [map, setMap] = useState<SystemDto>();
  const [error, setError] = useState<string>();
  const key = workspaceKey(workspace);
  const version = useSyncExternalStore(subscribe, () => versions.get(key) ?? 0);
  const reload = useCallback(() => bump(key), [key]);
  const saves = useViewSaves();

  useEffect(() => {
    let live = true;
    load(workspace, version, saves).then(
      (m) => {
        if (!live) return;
        setMap(m);
        setError(undefined);
      },
      (e) => live && setError(errorText(e)),
    );
    return () => {
      live = false;
    };
  }, [workspace, version, saves]);

  return { map, error, reload };
}
