import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect } from "react";
import { events } from "../bindings";
import { dockOf } from "../workbench/docks";
import { openReport } from "../workbench/panels";
import { type WorkspaceName, workspaceKey } from "../workspaces/context";

/** Opens a report a terminal agent published findings to. */
export function useAgentReports(workspace: WorkspaceName) {
  useEffect(() => {
    const key = workspaceKey(workspace);
    const sub = events.botRunChanged(getCurrentWebviewWindow()).listen(({ payload: p }) => {
      if (!p.show || workspaceKey(p.workspace) !== key) return;
      const api = dockOf(workspace);
      if (api) openReport(api, p.run);
    });
    return () => {
      sub.then((un) => un());
    };
  }, [workspace]);
}
