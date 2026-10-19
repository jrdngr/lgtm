import type { WorkspaceName } from "../core/workspace";
import type { CtxEntry } from "../ui/contextMenu";
import { pinTarget, sendTarget, workspaceTerminals } from "./registry";

/** Sends text to the send target, or to a chosen one. */
export interface Sender {
  recent(text: string): void;
  to(text: string, id: string): void;
}

/** "`label` to Terminal" sends `text` to the send target. With several terminals, "`label`
 * to" lists them, the send target first. */
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

/** Pins terminal `id` as its workspace's send target. */
export function targetItems(ws: WorkspaceName, id: string): CtxEntry[] {
  return [
    {
      label: "Pin as Send Target",
      checked: sendTarget(ws) === id,
      onSelect: () => pinTarget(ws, id),
    },
  ];
}
