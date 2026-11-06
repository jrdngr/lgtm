import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect } from "react";
import { type AgentActivity, events, type TurnEdit } from "../bindings";
import { type WorkspaceName, workspaceKey } from "../core/workspace";
import { commands } from "../ipc";
import { workspaceStore } from "../review/workspaceStore";

/** Each workspace's newest agent turn, or null when it has none. */
const store = workspaceStore<AgentActivity | null>();
const watched = new Set<string>();

async function reload(workspace: WorkspaceName) {
  const r = await commands.agentActivity(workspace);
  if (r.status === "ok") store.publish(workspace, r.data);
}

/** Starts keeping the workspace's activity current on agents' hooks. */
function watch(workspace: WorkspaceName) {
  const key = workspaceKey(workspace);
  if (watched.has(key)) return;
  watched.add(key);
  reload(workspace);
  events.turnsChanged(getCurrentWebviewWindow()).listen((e) => {
    if (workspaceKey(e.payload.workspace) === key) reload(workspace);
  });
}

export function useAgentActivity(workspace: WorkspaceName): AgentActivity | null | undefined {
  useEffect(() => watch(workspace), [workspace]);
  return store.use(workspace);
}

export interface HotFile {
  path: string;
  /** Edits that touched it. */
  edits: number;
  /** When it was last edited (Unix ms). */
  last: number;
}

/** The files a turn edited, most recently edited first. */
export function hotFiles(edits: TurnEdit[]): HotFile[] {
  const files = new Map<string, HotFile>();
  for (const e of edits) {
    for (const path of e.paths) {
      const f = files.get(path);
      files.set(path, {
        path,
        edits: (f?.edits ?? 0) + 1,
        last: Math.max(f?.last ?? 0, e.at ?? 0),
      });
    }
  }
  return [...files.values()].sort((a, b) => b.last - a.last || a.path.localeCompare(b.path));
}
