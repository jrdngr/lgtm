import type { WorkspaceName } from "../workspaces/context";

/** A live terminal pane that text can be pasted into. */
interface TerminalTarget {
  workspace: WorkspaceName;
  /** Its tab's title, else the title its program set, if any. */
  title(): string;
  /** False when the program in it can't take a paste safely. */
  canPaste(): boolean;
  paste(text: string): void;
  /** Brings the pane forward and focuses it. */
  focus(): void;
}

/** Terminals by panel id, most recently focused last. */
const terminals = new Map<string, TerminalTarget>();

export function registerTerminal(id: string, target: TerminalTarget) {
  terminals.set(id, target);
  return () => {
    terminals.delete(id);
  };
}

/** Moves the terminal to the front of the send order. */
export function touchTerminal(id: string) {
  const t = terminals.get(id);
  if (!t) return;
  terminals.delete(id);
  terminals.set(id, t);
}

export interface TerminalChoice {
  id: string;
  title: string;
  canPaste: boolean;
}

/** The workspace's terminals, most recently focused first. */
export function workspaceTerminals(workspace: WorkspaceName): TerminalChoice[] {
  return [...terminals]
    .filter(([, t]) => t.workspace === workspace)
    .reverse()
    .map(([id, t]) => ({ id, title: t.title(), canPaste: t.canPaste() }));
}

type SendResult = "sent" | "noTerminal" | "unsafe";

/** C0 and C1 control characters other than tab and newline. An escape in pasted text could end
 * bracketed paste early and have the shell run the rest. */
// biome-ignore lint/suspicious/noControlCharactersInRegex: matching them is the point
const CONTROLS = /[\x00-\x08\x0b-\x1f\x7f-\x9f]/g;

/** Pastes `text` into terminal `id`, or else the workspace's most recently focused one, and
 * focuses it. Nothing is sent when that terminal's program doesn't use bracketed paste, since
 * a plain shell would run each pasted line. */
export function sendToTerminal(workspace: WorkspaceName, text: string, id?: string): SendResult {
  const target =
    id === undefined
      ? [...terminals.values()].reverse().find((t) => t.workspace === workspace)
      : terminals.get(id);
  if (!target) return "noTerminal";
  if (!target.canPaste()) return "unsafe";
  target.paste(text.replace(CONTROLS, ""));
  target.focus();
  return "sent";
}
