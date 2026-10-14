import type { DockviewApi } from "dockview-react";
import type { PrDto, WorkspaceDto } from "../bindings";
import { commands, unwrap } from "../ipc";
import { currentReview } from "../review/current";
import { setWorkspaceRevset } from "../review/revset";
import { targetOf } from "../review/target";
import { showThread } from "../threads/focus";
import { type CtxEntry, copy } from "../ui/contextMenu";
import type { MenuItem } from "../ui/Menu";
import { pathItems, run } from "../ui/pathMenu";
import { dockReady } from "../workbench/docks";
import { addTerminal, focusPanel, isReviewPanel, openReviewTab } from "../workbench/panels";
import type { WorkspaceName } from "./context";

/** The row's Delete, in its ⋯ menu and its right-click menu. The main workspace can't go. */
export const deleteItem = (w: WorkspaceDto, onDelete: () => void): MenuItem => ({
  label: "Delete Workspace…",
  danger: true,
  disabled: w.name === null,
  onSelect: onDelete,
});

/** Shows the workspace's review on `@`, in its review pane or a new one. A pane still loading
 * opens on the workspace's revset. */
function reviewAt(workspace: WorkspaceName, api: DockviewApi) {
  setWorkspaceRevset(workspace, "@");
  const pane = api.panels.find(isReviewPanel);
  if (!pane) return openReviewTab(api);
  pane.api.setActive();
  targetOf(workspace)?.load("@");
}

interface Row {
  w: WorkspaceDto;
  /** Set in a PR workspace. */
  pr: PrDto | null | undefined;
  /** Whether the row shows the dot of a finished run. */
  dot: boolean;
  /** Switches to the workspace. */
  open(): void;
  onDelete(): void;
  onClearDot(): void;
}

/** A workspace row's right-click menu. Items that work in the workspace switch to it first. */
export function workspaceMenu({ w, pr, dot, open, onDelete, onClearDot }: Row): CtxEntry[] {
  const ws = w.name;
  const inIt = (f: (api: DockviewApi) => void) => () => {
    open();
    // A frame later, once the switch has shown the workspace's dock.
    dockReady(ws).then((api) => requestAnimationFrame(() => f(api)));
  };
  return [
    { label: "Open Workspace", onSelect: open },
    "-",
    { label: "New Terminal Here", onSelect: inIt((api) => focusPanel(addTerminal(api))) },
    pr
      ? { label: "Open Threads", keys: "⇧⌘T", onSelect: inIt(() => showThread(ws)) }
      : { label: "Open Review on @", onSelect: inIt((api) => reviewAt(ws, api)) },
    "-",
    ...(pr
      ? [
          {
            label: "Sync PR",
            disabled: !currentReview(ws),
            onSelect: () => currentReview(ws)?.refetchPr(),
          },
          { label: "Open PR on GitHub", onSelect: () => run(unwrap(commands.openPrPage(ws))) },
          { label: "Copy PR URL", onSelect: () => copy(pr.url) },
          "-" as const,
        ]
      : []),
    ...pathItems(ws, "", { relative: false }),
    "-",
    w.name !== null && !pr && dot && { label: "Clear Activity Dot", onSelect: onClearDot },
    "-",
    deleteItem(w, onDelete),
  ];
}
