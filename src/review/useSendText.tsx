import { useState } from "react";
import { sendToTerminal, type TerminalChoice, workspaceTerminals } from "../terminal/registry";
import type { WorkspaceName } from "../workspaces/context";
import { TerminalPicker } from "./TerminalPicker";

/** Where sent text went (the terminal's title, which may be empty), or why it couldn't go. */
export type SendOutcome = { terminal: string } | { error: string };
type Report = (outcome: SendOutcome) => void;

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
    onSent?: () => void;
    report?: Report;
  }>();

  /** Pastes text into the prompt of one of the workspace's terminals, asking which one when
   * there are several. Calls `onSent` once it's sent. With `report`, it's told which terminal
   * took the text or why none did, and failures don't show as the review's error. */
  function sendText(text: string, onSent?: () => void, report?: Report) {
    const terminals = workspaceTerminals(workspace);
    if (terminals.length > 1) setPicking({ text, terminals, onSent, report });
    else sendTo(text, undefined, onSent, report);
  }

  function sendTo(text: string, id: string | undefined, onSent?: () => void, report?: Report) {
    setPicking(undefined);
    const where = workspace ? `workspace ${workspace}` : "the main workspace";
    const terminal = workspaceTerminals(workspace).find((t) => id === undefined || t.id === id);
    const result = sendToTerminal(workspace, text, id);
    const error =
      result === "noTerminal"
        ? `No terminal is open in ${where}. Open one with ⌘T and start your agent.`
        : result === "unsafe"
          ? "That terminal can't take a paste safely: its program would run each line. Start your agent there first."
          : undefined;
    if (!report || !error) setError(error);
    report?.(error ? { error } : { terminal: terminal?.title ?? "" });
    if (!error) onSent?.();
  }

  const picker = picking && (
    <TerminalPicker
      terminals={picking.terminals}
      onPick={(id) => sendTo(picking.text, id, picking.onSent, picking.report)}
      onClose={() => {
        setPicking(undefined);
        focusRoot();
      }}
    />
  );
  return { sendText, picker };
}
