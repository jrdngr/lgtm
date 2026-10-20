import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useCallback, useEffect, useState } from "react";
import { events, type SystemDto } from "../bindings";
import { type WorkspaceName, workspaceKey } from "../core/workspace";
import { commands, errorText, unwrap } from "../ipc";
import { useViewSaves } from "./archView";

/** The workspace's system map, loaded again when a terminal agent describes it, the view
 * changes, or on `reload`.
 * Keeps the last map while the next loads. */
export function useSystem(workspace: WorkspaceName) {
  const [map, setMap] = useState<SystemDto>();
  const [error, setError] = useState<string>();
  const [version, setVersion] = useState(0);
  const reload = useCallback(() => setVersion((v) => v + 1), []);
  const saves = useViewSaves();

  // biome-ignore lint/correctness/useExhaustiveDependencies: `version` and `saves` ask for a reload
  useEffect(() => {
    let live = true;
    unwrap(commands.systemMap(workspace)).then(
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

  useEffect(() => {
    const key = workspaceKey(workspace);
    const sub = events.systemChanged(getCurrentWebviewWindow()).listen(({ payload: p }) => {
      if (workspaceKey(p.workspace) === key) reload();
    });
    return () => {
      sub.then((un) => un());
    };
  }, [workspace, reload]);

  return { map, error, reload };
}
