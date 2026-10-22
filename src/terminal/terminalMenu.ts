import type { Terminal } from "@xterm/xterm";
import type { DockviewApi, IDockviewPanel } from "dockview-react";
import { logger } from "../core/log";
import { addTerminal } from "../core/panels";
import type { WorkspaceName } from "../core/workspace";
import { type CtxEntry, clipboardText, copy } from "../ui/contextMenu";
import { moveToOther } from "../workbench/tabMenu";
import { targetItems } from "./sendItems";

const log = logger("terminal");

interface Deps {
  term: Terminal;
  workspace: WorkspaceName;
  /** The dock the terminal is in. */
  api: DockviewApi;
  panel: IDockviewPanel;
  inBottom: boolean;
}

/** The right-click menu of a terminal's screen. */
export function terminalMenu(d: Deps): CtxEntry[] {
  const { term, api, panel } = d;
  const after = (fn: () => void) => () => {
    fn();
    term.focus();
  };
  const split = (direction: "right" | "below") => () => {
    panel.api.setActive();
    addTerminal(api, direction);
  };
  return [
    {
      label: "Copy",
      keys: "⌘C",
      disabled: !term.hasSelection(),
      onSelect: after(() => copy(term.getSelection())),
    },
    {
      label: "Paste",
      keys: "⌘V",
      onSelect: after(() => {
        clipboardText().then(
          (text) => term.paste(text),
          (e) => log.error("paste", e),
        );
      }),
    },
    { label: "Select All", onSelect: after(() => term.selectAll()) },
    { label: "Clear", onSelect: after(() => term.clear()) },
    "-",
    ...targetItems(d.workspace, panel.id),
    "-",
    { label: "New Terminal", keys: "⌘T", onSelect: () => addTerminal(api) },
    { label: "Split Right", keys: "⌘D", onSelect: split("right") },
    { label: "Split Down", keys: "⇧⌘D", onSelect: split("below") },
    "-",
    {
      label: d.inBottom ? "Move to Main Area" : "Move to Bottom Panel",
      onSelect: () => moveToOther(d.workspace, d.panel, d.inBottom),
    },
    { label: "Close Terminal", onSelect: () => panel.api.close() },
  ];
}
