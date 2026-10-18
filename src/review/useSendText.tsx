import { message } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import type { WorkspaceName } from "../core/workspace";
import {
  sendTarget,
  sendToTerminal,
  type TerminalChoice,
  workspaceTerminals,
} from "../terminal/registry";
import type { Sender } from "../terminal/sendItems";
import { TerminalPicker } from "./TerminalPicker";

/** Where sent text went (the terminal's title, which may be empty), or why it couldn't go. */
export type SendOutcome = { terminal: string } | { error: string };
type Report = (outcome: SendOutcome) => void;
/** Told which terminal took the text. */
type Sent = (terminal: string) => void;

/** Sending text to one of the workspace's terminals. `picker` is the terminal picker to
 * render while the user chooses one. */
export function useSendText(
  workspace: WorkspaceName,
  setError: (message: string | undefined) => void,
  focusRoot: () => void,
) {
  /** Text waiting for the user to pick which terminal it goes to. */
  const [picking, setPicking] = useState<{
    text: string;
    terminals: TerminalChoice[];
    onSent?: Sent;
    report?: Report;
  }>();

  /** Pastes text into the prompt of `preferred` when it can take it, else the workspace's
   * send target, asking which terminal when neither can. Calls `onSent` once it's sent. With
   * `report`, it's told which terminal took the text or why none did, and failures don't show
   * as the review's error. */
  function sendText(text: string, onSent?: Sent, report?: Report, preferred?: string) {
    const terminals = workspaceTerminals(workspace);
    const usable = (id: string | undefined) => terminals.find((t) => t.id === id && t.canPaste);
    const to = usable(preferred) ?? usable(sendTarget(workspace));
    if (to) sendTo(text, to.id, onSent, report);
    else if (terminals.length > 1) setPicking({ text, terminals, onSent, report });
    else sendTo(text, undefined, onSent, report);
  }

  function sendTo(text: string, id: string | undefined, onSent?: Sent, report?: Report) {
    setPicking(undefined);
    const where = workspace ? `workspace ${workspace}` : "the main workspace";
    const to = id ?? sendTarget(workspace);
    const terminal = workspaceTerminals(workspace).find((t) => t.id === to);
    const result = sendToTerminal(workspace, text, to);
    const error =
      result === "noTerminal"
        ? `No terminal is open in ${where}. Open one with ⌘T and start your agent.`
        : result === "unsafe"
          ? "That terminal can't take a paste safely: its program would run each line. Start your agent there first."
          : undefined;
    if (!report || !error) setError(error);
    report?.(error ? { error } : { terminal: terminal?.title ?? "" });
    if (!error && to !== undefined) onSent?.(to);
    return error;
  }

  const picker = picking && (
    <TerminalPicker
      terminals={picking.terminals}
      onPick={(id) => {
        // A failed send leaves focus on the closed picker, so hand it back.
        if (sendTo(picking.text, id, picking.onSent, picking.report)) focusRoot();
      }}
      onClose={() => {
        setPicking(undefined);
        focusRoot();
      }}
    />
  );
  /** Pastes into the workspace's send target, without asking. */
  const sendRecent = (text: string, onSent?: () => void) => {
    sendTo(text, undefined, onSent);
  };
  /** Pastes into terminal `id`, without asking. */
  const sendToId = (text: string, id: string, onSent?: () => void) => {
    sendTo(text, id, onSent);
  };
  /** For menus: the send target, or a chosen one. */
  const sender: Sender = { recent: (t) => sendRecent(t), to: (t, id) => sendToId(t, id) };
  return { sendText, sendRecent, sendToId, sender, picker };
}

/** Sending text from a place with no error line of its own, so failures show in a dialog. */
export function useSendFromMenu(workspace: WorkspaceName) {
  return useSendText(
    workspace,
    (m) => {
      if (m) message(m, { kind: "error" });
    },
    () => {},
  );
}
