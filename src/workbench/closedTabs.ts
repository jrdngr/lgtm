import type { DockviewApi, IDockviewPanel } from "dockview-react";
import { flushSync } from "react-dom";
import { bottom } from "../core/bottom";
import { bottomDockOf, type DockSlot, dockOf } from "../core/docks";
import { focusPanel } from "../core/panels";
import { type WorkspaceName, workspaceKey } from "../core/workspace";
import { isMoving } from "./crossDock";

export interface Closed {
  slot: DockSlot;
  id: string;
  component: string;
  title?: string;
  params?: Record<string, unknown>;
  renderer: IDockviewPanel["api"]["renderer"];
  group?: string;
  index: number;
}

const LIMIT = 30;

/** Each workspace's closed tabs, the latest last. */
const history = new Map<string, Closed[]>();

/** Records the tabs `api` closes, so ⇧⌘T can reopen them. A tab dragged to another pane or
 * dock isn't closed. */
export function trackClosed(workspace: WorkspaceName, api: DockviewApi, slot: DockSlot) {
  /** Where each tab sits, as of the last layout change: a removed tab has left its group. */
  const where = new Map<string, { group: string; index: number }>();
  const locate = () => {
    where.clear();
    for (const g of api.groups) {
      for (const [index, p] of g.panels.entries()) where.set(p.id, { group: g.id, index });
    }
  };
  locate();
  return [
    api.onDidLayoutChange(locate),
    api.onDidRemovePanel((panel) => {
      if (isMoving(panel)) return;
      const at = where.get(panel.id);
      const entry: Closed = {
        slot,
        id: panel.id,
        component: panel.view.contentComponent,
        title: panel.title,
        params: panel.params,
        renderer: panel.api.renderer,
        group: at?.group,
        index: at?.index ?? 0,
      };
      // Moving a tab between panes removes it before adding it back.
      setTimeout(() => {
        if (!api.getPanel(entry.id)) push(workspace, entry);
      });
    }),
  ];
}

function push(workspace: WorkspaceName, entry: Closed) {
  const key = workspaceKey(workspace);
  const list = [...(history.get(key) ?? []).filter((x) => x.id !== entry.id), entry];
  history.set(key, list.slice(-LIMIT));
}

/** Reopens the workspace's most recently closed tab where it was, or in the dock's active
 * pane when its pane is gone. Each call goes one further back. */
export function reopenClosed(workspace: WorkspaceName) {
  const list = history.get(workspaceKey(workspace));
  const entry = list?.pop();
  if (!entry) return;
  if (entry.slot === "bottom") flushSync(() => bottom.set({ open: true }));
  const api = entry.slot === "bottom" ? bottomDockOf(workspace) : dockOf(workspace);
  if (!api) return;
  const open = api.getPanel(entry.id);
  if (open) return focusPanel(open);
  const group = entry.group ? api.getGroup(entry.group) : undefined;
  const terminal = entry.component === "terminal";
  const panel = api.addPanel({
    // A terminal comes back as a new shell, without rerunning the workspace's setup.
    id: terminal ? `terminal-${crypto.randomUUID()}` : entry.id,
    component: entry.component,
    title: entry.title,
    params: terminal ? {} : entry.params,
    renderer: entry.renderer,
    position: group ? { referenceGroup: group.id, index: entry.index } : undefined,
  });
  focusPanel(panel);
}
