import { useState } from "react";
import { sendToTerminal, type TerminalChoice, workspaceTerminals } from "../terminal/registry";
import type { WorkspaceName } from "../workspaces/context";
import { TerminalPicker } from "./TerminalPicker";

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
  }>();

  /** Pastes text into the prompt of one of the workspace's terminals, asking which one when
   * there are several. Calls `onSent` once it's sent. */
  function sendText(text: string, onSent?: () => void) {
    const terminals = workspaceTerminals(workspace);
    if (terminals.length > 1) setPicking({ text, terminals, onSent });
    else sendTo(text, undefined, onSent);
  }

  function sendTo(text: string, id: string | undefined, onSent?: () => void) {
    setPicking(undefined);
    const where = workspace ? `workspace ${workspace}` : "the main workspace";
    const result = sendToTerminal(workspace, text, id);
    if (result === "noTerminal") {
      setError(`No terminal is open in ${where}. Open one with ⌘T and start your agent.`);
      return;
    }
    if (result === "unsafe") {
      setError(
        "That terminal can't take a paste safely: its program would run each line. Start your agent there first.",
      );
      return;
    }
    setError(undefined);
    onSent?.();
  }

  const picker = picking && (
    <TerminalPicker
      terminals={picking.terminals}
      onPick={(id) => sendTo(picking.text, id, picking.onSent)}
      onClose={() => {
        setPicking(undefined);
        focusRoot();
      }}
    />
  );
  return { sendText, picker };
}
