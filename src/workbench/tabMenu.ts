import type { DockviewApi, DockviewGroupPanel, IDockviewPanel } from "dockview-react";
import { flushSync } from "react-dom";
import { bottom } from "../core/bottom";
import { bottomDockOf, dockOf } from "../core/docks";
import {
  addTerminal,
  addToGroup,
  focusPanel,
  kindTitle,
  type PanelKind,
  splitGroup,
} from "../core/panels";
import type { WorkspaceName } from "../core/workspace";
import type { EditorParams } from "../editor/open";
import { confirmClose, isUnsaved, saveEditor } from "../editor/unsaved";
import { commands, unwrap } from "../ipc";
import { currentReview } from "../review/current";
import { targetOf } from "../review/target";
import { pathItems, run } from "../services/pathMenu";
import { targetItems } from "../terminal/sendItems";
import { refreshName } from "../terminal/sessionTitle";
import { type CtxEntry, copy } from "../ui/contextMenu";
import { confirmAction } from "../ui/prompt";
import { moveAcross } from "./crossDock";
import { canMaximize, toggleMaximize } from "./maximize";
import { setRenaming } from "./tabRename";

/** Closes `panels` one by one, each editor asking about unsaved edits. Cancel stops there. */
async function closePanels(workspace: WorkspaceName, panels: IDockviewPanel[]) {
  for (const p of panels) {
    if (!(await confirmClose(workspace, p.id))) return;
    p.api.close();
  }
}

/** Moves `panel` to the workspace's other dock, showing the bottom panel to take it, or hiding
 * it when the move leaves it empty. */
export function moveToOther(workspace: WorkspaceName, panel: IDockviewPanel, inBottom: boolean) {
  const main = dockOf(workspace);
  const below = bottomDockOf(workspace);
  if (!main || !below) return;
  // Shown first, since a hidden terminal can't size itself.
  if (!inBottom) flushSync(() => bottom.set({ open: true }));
  const moved = inBottom
    ? moveAcross(panel, below, main, workspace)
    : moveAcross(panel, main, below, workspace);
  if (inBottom && below.panels.length === 0) bottom.set({ open: false });
  focusPanel(moved);
}

async function resetReview(workspace: WorkspaceName) {
  const review = currentReview(workspace);
  if (!review || !(await confirmAction("Clear every mark in this review?", "Reset"))) return;
  run(unwrap(commands.resetReviewMarks(review.id)).then(review.apply));
}

interface Tab {
  workspace: WorkspaceName;
  api: DockviewApi;
  panel: IDockviewPanel;
  inBottom: boolean;
  /** Closes the tab, asking first about unsaved edits. */
  close(): void;
}

/** A tab's right-click menu, by what the tab holds and which dock it's in. */
export function tabMenu({ workspace, api, panel, inBottom, close }: Tab): CtxEntry[] {
  const kind = panel.view.contentComponent;
  const terminal = kind === "terminal";
  const tabs = panel.group.panels;
  const at = tabs.indexOf(panel);
  const split = (direction: "right" | "below") => () => {
    if (!terminal) return splitGroup(api, panel.group, direction);
    panel.api.setActive();
    addTerminal(api, direction);
  };
  const path = kind === "editor" ? (panel.params as EditorParams | undefined)?.path : undefined;
  return [
    terminal && { label: "Rename", onSelect: () => setRenaming(panel.id) },
    terminal && { label: "Refresh Name", onSelect: () => refreshName(panel) },
    "-",
    ...(terminal ? targetItems(workspace, panel.id) : []),
    path && {
      label: "Save",
      keys: "⌘S",
      disabled: !isUnsaved(workspace, panel.id),
      onSelect: () => saveEditor(workspace, panel.id),
    },
    "-",
    { label: "Close", onSelect: close },
    {
      label: "Close Others",
      disabled: tabs.length < 2,
      onSelect: () =>
        closePanels(
          workspace,
          tabs.filter((p) => p !== panel),
        ),
    },
    {
      label: "Close to the Right",
      disabled: at === tabs.length - 1,
      onSelect: () => closePanels(workspace, tabs.slice(at + 1)),
    },
    !inBottom && { label: "Close All", onSelect: () => closePanels(workspace, [...tabs]) },
    "-",
    { label: "Split Right", keys: terminal ? "⌘D" : undefined, onSelect: split("right") },
    { label: "Split Down", keys: terminal ? "⇧⌘D" : undefined, onSelect: split("below") },
    {
      label: inBottom ? "Move to Main Area" : "Move to Bottom Panel",
      disabled: isUnsaved(workspace, panel.id),
      onSelect: () => moveToOther(workspace, panel, inBottom),
    },
    "-",
    ...(kind === "review"
      ? [
          {
            label: "Copy Revset",
            onSelect: () => {
              const revset = targetOf(workspace)?.revset;
              if (revset) copy(revset);
            },
          },
          { label: "Reset Review…", onSelect: () => resetReview(workspace) },
        ]
      : []),
    ...(path ? pathItems(workspace, path, { files: true }) : []),
  ];
}

/** What the new tab submenu offers, in order. */
const NEW_TABS: PanelKind[] = ["claude", "terminal", "review", "guided", "architecture", "qa"];

interface Strip {
  workspace: WorkspaceName;
  api: DockviewApi;
  group: DockviewGroupPanel;
  inBottom: boolean;
}

/** The menu of the space after a pane's tabs. */
export function stripMenu({ workspace, api, group, inBottom }: Strip): CtxEntry[] {
  const newTab: CtxEntry = {
    label: "New Tab",
    items: NEW_TABS.map((kind) => ({
      label: kindTitle(kind),
      keys: kind === "terminal" ? "⌘T" : undefined,
      onSelect: () => addToGroup(api, group, kind),
    })),
  };
  if (inBottom) {
    return [
      {
        label: "New Terminal",
        keys: "⌃⇧`",
        onSelect: () => addToGroup(api, group, "terminal"),
      },
      newTab,
      "-",
      {
        label: "Collapse Bottom Panel",
        keys: "⌃`",
        onSelect: () => {
          bottom.set({ open: false });
          focusPanel(dockOf(workspace)?.activePanel);
        },
      },
    ];
  }
  return [
    newTab,
    "-",
    { label: "Split Right", onSelect: () => splitGroup(api, group, "right") },
    { label: "Split Down", onSelect: () => splitGroup(api, group, "below") },
    {
      label: api.hasMaximizedGroup() ? "Restore Panes" : "Maximize Pane",
      keys: "⇧⌘↵",
      disabled: !canMaximize(api, group),
      onSelect: () => toggleMaximize(api, group),
    },
    "-",
    {
      label: "Close All Tabs in Pane",
      disabled: group.panels.length === 0,
      onSelect: () => closePanels(workspace, [...group.panels]),
    },
  ];
}
