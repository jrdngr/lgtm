import type { WorkspaceName } from "../core/workspace";
import type { CtxEntry } from "../ui/contextMenu";
import { pinTarget, roleOf, workspaceTerminals } from "./registry";

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

/** Pins terminal `id` as its workspace's interactive or background terminal. */
export function targetItems(ws: WorkspaceName, id: string): CtxEntry[] {
  const role = roleOf(ws, id);
  return [
    {
      label: "Pin as Interactive",
      checked: role === "interactive",
      onSelect: () => pinTarget(ws, id, "interactive"),
    },
    {
      label: "Pin as Background",
      checked: role === "background",
      onSelect: () => pinTarget(ws, id, "background"),
    },
  ];
}
