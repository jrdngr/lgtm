import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect } from "react";
import { events } from "../bindings";
import { type WorkspaceName, workspaceKey } from "../core/workspace";
import { reloadArch } from "./state";

/** Loads a review's architecture again when a terminal agent assesses it. */
export function useAgentArch(workspace: WorkspaceName) {
  useEffect(() => {
    const key = workspaceKey(workspace);
    const sub = events.archChanged(getCurrentWebviewWindow()).listen(({ payload: p }) => {
      if (workspaceKey(p.workspace) === key) reloadArch(p.review);
    });
    return () => {
      sub.then((un) => un());
    };
  }, [workspace]);
}
