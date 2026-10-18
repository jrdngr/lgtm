import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect, useRef } from "react";
import { events } from "../bindings";
import { dockOf, revealPanel } from "../core/docks";
import type { WorkspaceName } from "../core/workspace";

/** The Settings tab's sections, which panels can open it at. */
export type SettingsSection = "claude" | "terminal" | "tools" | "prompts";

export interface SettingsParams {
  section?: SettingsSection;
  /** Changes on every open, so opening it again scrolls to the section again. */
  nonce?: number;
}

const ID = "settings";

/** Opens the Settings tab in the workspace's dock, or focuses it, at `section`. */
export function openSettings(workspace: WorkspaceName, section?: SettingsSection) {
  const api = dockOf(workspace);
  if (!api) return;
  const params: SettingsParams = { section, nonce: Date.now() };
  const open = revealPanel(workspace, ID);
  if (open) return open.api.updateParameters(params);
  const active = api.activePanel;
  api.addPanel({
    id: ID,
    component: "settings",
    title: "Settings",
    params,
    position: active ? { referencePanel: active, direction: "within" } : undefined,
  });
}

/** lgtm > Settings… opens the tab in the selected workspace. */
export function useSettingsMenu(selected: WorkspaceName) {
  const current = useRef(selected);
  current.current = selected;
  useEffect(() => {
    const sub = events
      .openSettings(getCurrentWebviewWindow())
      .listen(() => openSettings(current.current));
    return () => {
      sub.then((stop) => stop());
    };
  }, []);
}
