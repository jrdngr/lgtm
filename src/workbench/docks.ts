import type { DockviewApi } from "dockview-react";
import { flushSync } from "react-dom";
import { type WorkspaceName, workspaceKey } from "../workspaces/context";
import { bottom } from "./bottom";

export type DockSlot = "main" | "bottom";

/** Each mounted workspace's docks, for opening panes from outside them. */
const docks: Record<DockSlot, Map<string, DockviewApi>> = { main: new Map(), bottom: new Map() };
/** Callers waiting for a workspace's main dock to come up. */
const waiting = new Map<string, ((api: DockviewApi) => void)[]>();

export function registerDock(
  workspace: WorkspaceName,
  api: DockviewApi | undefined,
  slot: DockSlot = "main",
) {
  const key = workspaceKey(workspace);
  if (!api) {
    docks[slot].delete(key);
    return;
  }
  docks[slot].set(key, api);
  if (slot !== "main") return;
  for (const done of waiting.get(key) ?? []) done(api);
  waiting.delete(key);
}

/** The workspace's main dock, where panes open from outside it. */
export const dockOf = (workspace: WorkspaceName) => docks.main.get(workspaceKey(workspace));

/** The workspace's main dock, once it's up: a workspace just selected mounts its dock first. */
export function dockReady(workspace: WorkspaceName): Promise<DockviewApi> {
  const api = dockOf(workspace);
  if (api) return Promise.resolve(api);
  const key = workspaceKey(workspace);
  return new Promise((done) => waiting.set(key, [...(waiting.get(key) ?? []), done]));
}

export const bottomDockOf = (workspace: WorkspaceName) => docks.bottom.get(workspaceKey(workspace));

/** The panel `id` in the workspace's main dock, or else its bottom dock. */
const panelIn = (workspace: WorkspaceName, id: string) =>
  dockOf(workspace)?.getPanel(id) ?? bottomDockOf(workspace)?.getPanel(id);

/** Whether the panel `id` is in one of the workspace's docks, or the main dock isn't up yet. */
export function inDock(workspace: WorkspaceName, id: string) {
  return !dockOf(workspace) || !!panelIn(workspace, id);
}

/** Whether the panel `id` is in one of the workspace's docks. */
export const hasPanel = (workspace: WorkspaceName, id: string) => !!panelIn(workspace, id);

/** Brings the panel `id` forward in whichever of the workspace's docks has it, showing the
 * bottom panel when it's there. Returns the panel, or undefined when neither dock has it. */
export function revealPanel(workspace: WorkspaceName, id: string) {
  let panel = dockOf(workspace)?.getPanel(id);
  if (!panel) {
    panel = bottomDockOf(workspace)?.getPanel(id);
    if (!panel) return undefined;
    // Shown first, since a hidden dock can't lay out or focus the panel.
    flushSync(() => bottom.set({ open: true }));
  }
  panel.api.setActive();
  return panel;
}
