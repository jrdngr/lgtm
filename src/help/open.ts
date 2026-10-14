import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect, useRef } from "react";
import { events } from "../bindings";
import { dockOf } from "../workbench/docks";
import { activate } from "../workbench/panels";
import type { WorkspaceName } from "../workspaces/context";

const ID = "shortcuts";

/** Opens the Keyboard Shortcuts tab in the workspace's dock, or focuses it. */
export function openShortcuts(workspace: WorkspaceName) {
  const api = dockOf(workspace);
  if (!api) return;
  const open = api.getPanel(ID);
  if (open) return activate(open);
  const active = api.activePanel;
  api.addPanel({
    id: ID,
    component: "shortcuts",
    title: "Keyboard Shortcuts",
    position: active ? { referencePanel: active, direction: "within" } : undefined,
  });
}

/** Help › Keyboard Shortcuts opens the tab in the selected workspace. */
export function useShortcutsMenu(selected: WorkspaceName) {
  const current = useRef(selected);
  current.current = selected;
  useEffect(() => {
    const sub = events
      .openShortcuts(getCurrentWebviewWindow())
      .listen(() => openShortcuts(current.current));
    return () => {
      sub.then((stop) => stop());
    };
  }, []);
}
