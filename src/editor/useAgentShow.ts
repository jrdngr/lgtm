import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect } from "react";
import { events } from "../bindings";
import { currentReview } from "../review/current";
import type { WorkspaceName } from "../workspaces/context";
import { openFile } from "./open";

/** Opens files agents ask to see in the editor while the workspace's review hasn't loaded,
 * which otherwise shows them. */
export function useAgentShow(workspace: WorkspaceName) {
  useEffect(() => {
    const sub = events.agentShow(getCurrentWebviewWindow()).listen(({ payload: p }) => {
      if (p.workspace === workspace && !currentReview(workspace)) {
        openFile(workspace, p.path, p.line ?? undefined);
      }
    });
    return () => {
      sub.then((un) => un());
    };
  }, [workspace]);
}
