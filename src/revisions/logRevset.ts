import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { events } from "../bindings";
import type { WorkspaceName } from "../core/workspace";
import { workspaceStore } from "../review/workspaceStore";

/** The revset a workspace's revisions log lists, `null` for jj's default, and jj's error when
 * the log fell back to the default. Published by the log as it loads. */
export interface LogRevset {
  revset: string | null;
  error: string | null;
}

const store = workspaceStore<LogRevset>();

export const publishLogRevset = store.publish;
export const useLogRevset = store.use;

/** Calls `f` when the workspace's log revset changes, from the strip or an agent. */
export function onLogRevsetChanged(workspace: WorkspaceName, f: () => void) {
  const un = events.logRevsetChanged(getCurrentWebviewWindow()).listen(({ payload }) => {
    if (payload.workspace === workspace) f();
  });
  return () => {
    un.then((u) => u());
  };
}
