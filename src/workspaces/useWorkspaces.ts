import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect, useRef, useState } from "react";
import { events, type ProgressDto, type WorkspaceDto } from "../bindings";
import { commands, unwrap } from "../ipc";
import { workspaceKey } from "./context";

const listeners = new Set<() => void>();

/** Tells the workspace list that marks changed, which no jj operation reports. */
export function marksChanged() {
  for (const l of listeners) l();
}

/** The repo's workspaces and each one's review progress, kept current as the repo changes.
 * Reloads requested while one runs coalesce into one more, a second later. */
export function useWorkspaces() {
  const [workspaces, setWorkspaces] = useState<WorkspaceDto[]>();
  const [progress, setProgress] = useState<ReadonlyMap<string, ProgressDto>>(new Map());
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
      setError(undefined);
      const entries = await Promise.all(
        list.map(async (t) => {
          const r = await commands.workspaceProgress(t.name);
          return r.status === "ok" ? ([workspaceKey(t.name), r.data] as const) : undefined;
        }),
      );
      setProgress(new Map(entries.filter((e) => e !== undefined)));
    } catch (e) {
      setError(String(e));
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
    const sub = events.repoChanged(getCurrentWebviewWindow()).listen(onChange);
    return () => {
      listeners.delete(onChange);
      sub.then((un) => un());
    };
  }, []);

  return { workspaces, progress, error, setError, reload };
}
