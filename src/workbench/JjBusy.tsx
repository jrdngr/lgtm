import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect } from "react";
import { events } from "../bindings";
import type { WorkspaceName } from "../core/workspace";
import { workspaceStore } from "../review/workspaceStore";
import "./JjBusy.css";

/** Workspaces whose background snapshots are paused after giving up on a jj lock. */
const paused = workspaceStore<boolean>();
/** What has the jj lock a workspace's write is waiting on, one process per line. */
const blocked = workspaceStore<string>();

/** Keeps each workspace's jj lock state as the backend reports it. */
export function useTrackJjBusy() {
  useEffect(() => {
    // Scoped to this window: other windows hold other repos.
    const win = getCurrentWebviewWindow();
    const pausedSub = events.jjPaused(win).listen((e) => {
      const { workspace, paused: on } = e.payload;
      if (on) paused.publish(workspace, true);
      else paused.retract(workspace, true);
    });
    const blockedSub = events.jjBlocked(win).listen((e) => {
      const { workspace, holders } = e.payload;
      if (holders !== null) blocked.publish(workspace, holders);
      else blocked.retract(workspace, blocked.get(workspace));
    });
    return () => {
      pausedSub.then((un) => un());
      blockedSub.then((un) => un());
    };
  }, []);
}

/** In the header while the workspace's background snapshots are paused. */
export function JjBusyTag({ workspace }: { workspace: WorkspaceName }) {
  return paused.use(workspace) ? <span className="jj-busy">jj busy</span> : null;
}

/** Under the header while a write in the workspace waits on a jj lock: what has it open. */
export function JjBlockedBanner({ workspace }: { workspace: WorkspaceName }) {
  const holders = blocked.use(workspace);
  if (holders === undefined) return null;
  return (
    <div className="jj-blocked">
      <span>Waiting on jj lock</span>
      {holders.split("\n").map(
        (line) =>
          line && (
            <code key={line} className="jj-holder">
              {line}
            </code>
          ),
      )}
    </div>
  );
}
