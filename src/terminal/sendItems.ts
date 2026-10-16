import type { CtxEntry } from "../ui/contextMenu";
import type { WorkspaceName } from "../workspaces/context";
import { workspaceTerminals } from "./registry";

/** Sends text to the last used terminal, or to a chosen one. */
export interface Sender {
  recent(text: string): void;
  to(text: string, id: string): void;
}

/** "`label` to Terminal" sends `text` to the last used terminal. With several terminals,
 * "`label` to" lists them, last used first. */
export function sendItems(
  ws: WorkspaceName,
  label: string,
  text: string,
  send: Sender,
): CtxEntry[] {
  const terminals = workspaceTerminals(ws);
  return [
    { label: `${label} to Terminal`, onSelect: () => send.recent(text) },
    terminals.length > 1 && {
      label: `${label} to`,
      items: terminals.map((t, i) => ({
        label: t.title || `Terminal ${i + 1}`,
        disabled: !t.canPaste,
        onSelect: () => send.to(text, t.id),
      })),
    },
  ];
}
