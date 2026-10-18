import {
  type DockviewApi,
  type DockviewGroupPanel,
  type IDockviewPanel,
  type Position,
  positionToDirection,
} from "dockview-react";
import type { WorkspaceName } from "../core/workspace";
import { isUnsaved } from "../editor/unsaved";

/** Panels being closed only to reopen in the other dock. */
const moving = new Set<IDockviewPanel>();

/** Whether `panel` left its dock for the other one, rather than being closed. */
export const isMoving = (panel: IDockviewPanel) => moving.has(panel);

/** Lets `api` take tabs dragged from `other`, the workspace's other dock. Dockview only moves
 * tabs within one dock, so a tab from the other is closed there and reopened here. Terminals
 * keep their shell across it; an editor with unsaved edits can't move. */
export function takeTabsFrom(
  api: DockviewApi,
  other: () => DockviewApi | undefined,
  workspace: WorkspaceName,
) {
  /** The dragged tabs, when they come from `other` and may move. */
  function dragged(data: { viewId: string; groupId: string; panelId: string | null } | undefined) {
    const from = other();
    if (!data || !from || data.viewId !== from.id) return undefined;
    const panels = data.panelId
      ? [from.getPanel(data.panelId)]
      : (from.getGroup(data.groupId)?.panels ?? []);
    const all = panels.filter((p): p is IDockviewPanel => !!p);
    if (all.length === 0 || all.some((p) => isUnsaved(workspace, p.id))) return undefined;
    return { from, panels: all };
  }

  return [
    api.onUnhandledDragOver((e) => {
      if (dragged(e.getData())) e.accept();
    }),
    api.onDidDrop((e) => {
      const drag = dragged(e.getData());
      if (!drag || !e.group) return;
      let into = e.group;
      let position = e.position;
      for (const panel of drag.panels) {
        into = move(panel, api, into, position);
        // The rest join the first, in the pane it went to.
        position = "center";
      }
      // An emptied dock keeps a pane to drop tabs back into.
      if (drag.from.groups.length === 0) drag.from.addGroup();
    }),
  ];
}

/** Moves `panel` to `position` in `group` of the dock `to`. Returns the group it's in. */
function move(
  panel: IDockviewPanel,
  to: DockviewApi,
  group: DockviewGroupPanel,
  position: Position,
): DockviewGroupPanel {
  const { id, title, params } = panel;
  const component = panel.view.contentComponent;
  const renderer = panel.api.renderer;
  moving.add(panel);
  panel.api.close();
  moving.delete(panel);
  // Panes that are one per dock, such as Architecture, are already here: show that one.
  const existing = to.getPanel(id);
  if (existing) {
    existing.api.setActive();
    return existing.group;
  }
  const added = to.addPanel({
    id,
    component,
    title,
    params,
    renderer,
    position: { referenceGroup: group, direction: positionToDirection(position) },
  });
  return added.group;
}

/** Moves `panel` from dock `from` into the active pane of `to`. Returns the moved panel, or
 * undefined for an editor with unsaved edits, which stays put. */
export function moveAcross(
  panel: IDockviewPanel,
  from: DockviewApi,
  to: DockviewApi,
  workspace: WorkspaceName,
): IDockviewPanel | undefined {
  if (isUnsaved(workspace, panel.id)) return undefined;
  const group = to.activeGroup ?? to.groups[0] ?? to.addGroup();
  move(panel, to, group, "center");
  if (from.groups.length === 0) from.addGroup();
  return to.getPanel(panel.id);
}
