import { message } from "@tauri-apps/plugin-dialog";
import type { WorkspaceName } from "../core/workspace";
import { openFile } from "../editor/open";
import { commands, unwrap } from "../ipc";
import { currentReview } from "../review/current";
import { keepSubmit, sendToTerminal, submitNextSend } from "../terminal/registry";
import type { CtxEntry } from "../ui/contextMenu";
import { findReferences, goToDefinition, searchWorkspace } from "./store";

/** Asks the send target's agent for Symbol Information on `name`, defined in `path` when it
 * is there, and shows the definition when the review doesn't. */
export async function askSymbolInfo(ws: WorkspaceName, name: string, path?: string) {
  submitNextSend();
  try {
    const ask = await keepSubmit(unwrap(commands.requestSymbolInfo(ws, name, path ?? null)));
    const sent = sendToTerminal(ws, ask.prompt);
    if (sent !== "sent")
      await message(
        sent === "noTerminal"
          ? "No terminal is open. Open one with ⌘T and start your agent."
          : "That terminal can't take a paste safely. Start your agent there first.",
      );
    const inReview = currentReview(ws)?.files.some((f) => f.dto.path === ask.path);
    if (!inReview) openFile(ws, ask.path, ask.line);
  } catch (e) {
    await message(String(e));
  }
}

/** The right-click items for the symbol `word`, ending in a separator. */
export function searchItems(ws: WorkspaceName, word: string, path?: string): CtxEntry[] {
  const none = !word;
  const short = word.length > 24 ? `${word.slice(0, 24)}…` : word;
  return [
    {
      label: "Go to Definition",
      keys: "F12",
      disabled: none,
      onSelect: () => goToDefinition(ws, word),
    },
    {
      label: "Find All References",
      keys: "⇧F12",
      disabled: none,
      onSelect: () => findReferences(ws, word),
    },
    {
      label: word ? `Search Workspace for "${short}"` : "Search Workspace",
      keys: "⇧⌘F",
      onSelect: () => searchWorkspace(ws, word),
    },
    {
      label: "Symbol Information",
      keys: "⌥F12",
      disabled: none,
      onSelect: () => askSymbolInfo(ws, word, path),
    },
    "-",
  ];
}
