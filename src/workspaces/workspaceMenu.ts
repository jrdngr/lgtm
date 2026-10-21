import type { DockviewApi } from "dockview-react";
import type { PrDto, WorkspaceDto } from "../bindings";
import { dockReady } from "../core/docks";
import { activate, addTerminal, focusPanel, isReviewPanel, openReviewTab } from "../core/panels";
import { type WorkspaceName, workspaceKey } from "../core/workspace";
import { commands, unwrap } from "../ipc";
import { currentReview } from "../review/current";
import { setWorkspaceRevset } from "../review/revset";
import { targetOf } from "../review/target";
import { pathItems, run } from "../services/pathMenu";
import { type CtxEntry, copy } from "../ui/contextMenu";
import type { MenuItem } from "../ui/Menu";
import { COLORS, type PinColor, setColor, togglePin } from "./pins";

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
  activate(pane);
  targetOf(workspace)?.load("@");
}

const title = (c: string) => c[0].toUpperCase() + c.slice(1);

function colorItems(key: string, color: PinColor | undefined): CtxEntry[] {
  return [
    { label: "None", checked: !color, onSelect: () => setColor(key, null) },
    "-",
    ...(Object.keys(COLORS) as PinColor[]).map((c) => ({
      label: title(c),
      swatch: COLORS[c],
      checked: c === color,
      onSelect: () => setColor(key, c),
    })),
  ];
}

interface Row {
  w: WorkspaceDto;
  pinned: boolean;
  color: PinColor | undefined;
  /** Set in a PR workspace. */
  pr: PrDto | null | undefined;
  /** Whether the row shows an agent that ended while away. */
  dot: boolean;
  /** Switches to the workspace. */
  open(): void;
  onDelete(): void;
  onClearDot(): void;
}

/** A workspace row's right-click menu. Items that work in the workspace switch to it first. */
export function workspaceMenu({
  w,
  pinned,
  color,
  pr,
  dot,
  open,
  onDelete,
  onClearDot,
}: Row): CtxEntry[] {
  const ws = w.name;
  const key = workspaceKey(ws);
  const inIt = (f: (api: DockviewApi) => void) => () => {
    open();
    // A frame later, once the switch has shown the workspace's dock.
    dockReady(ws).then((api) => requestAnimationFrame(() => f(api)));
  };
  return [
    { label: "Open Workspace", onSelect: open },
    "-",
    { label: pinned ? "Unpin" : "Pin to Top", onSelect: () => togglePin(key) },
    { label: "Color", items: colorItems(key, color) },
    "-",
    { label: "New Terminal Here", onSelect: inIt((api) => focusPanel(addTerminal(api))) },
    (!pr || pr.stack) && { label: "Open Review on @", onSelect: inIt((api) => reviewAt(ws, api)) },
    "-",
    ...(pr
      ? [
          !pr.stack && {
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
    w.name !== null && !pr && dot && { label: "Mark Agents Seen", onSelect: onClearDot },
    "-",
    deleteItem(w, onDelete),
  ];
}
