import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect, useRef } from "react";
import { events } from "../bindings";
import type { WorkspaceName } from "../workspaces/context";

interface Handlers {
  /** Whether this pane is the one its workspace's agents act on. */
  isCurrent(): boolean;
  show(path: string, line: number | null): void;
  load(revset: string): void;
}

/** Shows code and loads revsets when an agent in one of the workspace's terminals asks. */
export function useAgentRequests(workspace: WorkspaceName, handlers: Handlers) {
  const latest = useRef(handlers);
  latest.current = handlers;
  useEffect(() => {
    const win = getCurrentWebviewWindow();
    const mine = (w: string | null) => w === workspace && latest.current.isCurrent();
    const show = events.agentShow(win).listen(({ payload: p }) => {
      if (mine(p.workspace)) latest.current.show(p.path, p.line);
    });
    const revset = events.agentRevset(win).listen(({ payload: p }) => {
      if (mine(p.workspace)) latest.current.load(p.revset);
    });
    return () => {
      show.then((un) => un());
      revset.then((un) => un());
    };
  }, [workspace]);
}
