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

/** The row's Sleep, or Wake when it's asleep, in its ⋯ menu and its right-click menu. Sleep
 * is off when `onSleep` is unset. */
export const sleepItem = (w: WorkspaceDto, open: () => void, onSleep?: () => void): MenuItem =>
  w.asleep
    ? { label: "Wake", onSelect: open }
    : { label: "Sleep", disabled: !onSleep, onSelect: () => onSleep?.() };

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

/** A Color submenu, checking `color`. */
export function colorItems(
  color: PinColor | undefined,
  pick: (c: PinColor | null) => void,
): CtxEntry[] {
  return [
    { label: "None", checked: !color, onSelect: () => pick(null) },
    "-",
    ...(Object.keys(COLORS) as PinColor[]).map((c) => ({
      label: title(c),
      swatch: COLORS[c],
      checked: c === color,
      onSelect: () => pick(c),
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
  /** Switches to the workspace, which wakes it when it's asleep. */
  open(): void;
  /** Puts it to sleep. Unset when it can't sleep, as the only awake workspace. */
  onSleep?(): void;
  onDelete(): void;
  onLocate(): void;
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
  onSleep,
  onDelete,
  onLocate,
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
    { label: "Color", items: colorItems(color, (c) => setColor(key, c)) },
    "-",
    !w.missing && {
      label: "New Terminal Here",
      onSelect: inIt((api) => focusPanel(addTerminal(api))),
    },
    !w.missing &&
      (!pr || pr.stack) && {
        label: "Open Review on @",
        onSelect: inIt((api) => reviewAt(ws, api)),
      },
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
    ...(w.missing ? [] : pathItems(ws, "", { relative: false })),
    w.name !== null && { label: "Locate Folder…", onSelect: onLocate },
    "-",
    w.name !== null && !pr && dot && { label: "Mark Agents Seen", onSelect: onClearDot },
    "-",
    sleepItem(w, open, onSleep),
    "-",
    deleteItem(w, onDelete),
  ];
}
