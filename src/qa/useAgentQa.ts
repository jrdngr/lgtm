import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect } from "react";
import { events } from "../bindings";
import { type WorkspaceName, workspaceKey } from "../core/workspace";
import { loadSessions, showQa } from "./store";

/** Opens the Q&A tab on a session a terminal agent asked questions in. */
export function useAgentQa(workspace: WorkspaceName) {
  useEffect(() => {
    const key = workspaceKey(workspace);
    const sub = events.qaChanged(getCurrentWebviewWindow()).listen(({ payload: p }) => {
      if (!p.show || workspaceKey(p.workspace) !== key) return;
      loadSessions(workspace).then(() => showQa(workspace, p.id));
    });
    return () => {
      sub.then((un) => un());
    };
  }, [workspace]);
}
