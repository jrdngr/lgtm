import type { DockviewApi } from "dockview-react";
import { flushSync } from "react-dom";
import { bottom } from "./bottom";
import { activate } from "./panels";
import { type WorkspaceName, workspaceKey } from "./workspace";

export type DockSlot = "main" | "bottom";

/** Each mounted workspace's docks, for opening panes from outside them. */
const docks: Record<DockSlot, Map<string, DockviewApi>> = { main: new Map(), bottom: new Map() };
/** Bumped whenever a dock comes or goes. */
let version = 0;
const dockListeners = new Set<() => void>();

/** Calls `l` whenever a dock comes or goes. */
export function subscribeDocks(l: () => void) {
  dockListeners.add(l);
  return () => {
    dockListeners.delete(l);
  };
}

export const docksVersion = () => version;

/** Callers waiting for a workspace's main dock to come up. */
const waiting = new Map<string, ((api: DockviewApi) => void)[]>();

export function registerDock(
  workspace: WorkspaceName,
  api: DockviewApi | undefined,
  slot: DockSlot = "main",
) {
  const key = workspaceKey(workspace);
  version++;
  if (api) docks[slot].set(key, api);
  else docks[slot].delete(key);
  for (const l of dockListeners) l();
  if (!api) return;
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
  activate(panel);
  return panel;
}
