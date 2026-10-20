import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect } from "react";
import { events } from "../bindings";
import { dockOf } from "../core/docks";
import { openArchitecture } from "../core/panels";
import { type WorkspaceName, workspaceKey } from "../core/workspace";
import { showDesign } from "../design/designs";

/** Shows the Architecture tab when a terminal agent hands back a description or a tour of the
 * workspace's system, or makes or maps a design. */
export function useAgentSystem(workspace: WorkspaceName) {
  useEffect(() => {
    const key = workspaceKey(workspace);
    const sub = events.systemChanged(getCurrentWebviewWindow()).listen(({ payload: p }) => {
      if (workspaceKey(p.workspace) !== key) return;
      const api = dockOf(workspace);
      if (api) openArchitecture(api);
    });
    const design = events.designChanged(getCurrentWebviewWindow()).listen(({ payload: p }) => {
      if (workspaceKey(p.workspace) !== key) return;
      showDesign(p.id);
      const api = dockOf(workspace);
      if (api) openArchitecture(api);
    });
    return () => {
      sub.then((un) => un());
      design.then((un) => un());
    };
  }, [workspace]);
}
