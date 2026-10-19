import { useSyncExternalStore } from "react";
import { loadState, saveState } from "../core/savedState";
import { type WorkspaceName, workspaceKey } from "../core/workspace";

/** A live terminal pane that text can be pasted into. */
interface TerminalTarget {
  workspace: WorkspaceName;
  /** Its tab's title, else the title its program set, if any. */
  title(): string;
  /** False when the program in it can't take a paste safely. */
  canPaste(): boolean;
  paste(text: string): void;
  /** Whether an agent like Claude Code runs in it, by its hooks or, in a Claude tab, its title. */
  hasAgent(): boolean;
  /** Whether its agent is working or waiting on the user, or was just prompted. */
  agentBusy(): boolean;
  /** Marks its agent busy until its hooks report on the prompt. */
  prompted(): void;
  /** Presses Enter. */
  submit(): void;
  /** Stops its agent's turn. */
  interrupt(): void;
  /** Brings the pane forward and focuses it. */
  focus(): void;
}

/** Terminals by panel id, most recently focused last. */
const terminals = new Map<string, TerminalTarget>();
/** When each terminal opened, as a running count. */
const opened = new Map<string, number>();
let opens = 0;

/** Each workspace's pinned send target, by workspace key. Saved per repo. */
type Pins = Record<string, string>;
const PINS = "send-pins";
const isPins = (v: unknown): v is Pins =>
  typeof v === "object" && v !== null && Object.values(v).every((id) => typeof id === "string");
let pins: Pins | undefined;
const pinsNow = () => {
  pins ??= loadState(PINS, {}, isPins);
  return pins;
};

const listeners = new Set<() => void>();
/** Bumped on every change to the terminals, their order or the pins. */
let version = 0;
function changed() {
  version++;
  for (const l of listeners) l();
}

export function registerTerminal(id: string, target: TerminalTarget) {
  terminals.set(id, target);
  opened.set(id, opens++);
  changed();
  return () => {
    terminals.delete(id);
    opened.delete(id);
    changed();
  };
}

/** Moves the terminal to the front of the send order. */
export function touchTerminal(id: string) {
  const t = terminals.get(id);
  if (!t || [...terminals.keys()].at(-1) === id) return;
  terminals.delete(id);
  terminals.set(id, t);
  changed();
}

/** Pins terminal `id` as the workspace's send target. */
export function pinTarget(workspace: WorkspaceName, id: string) {
  const next = { ...pinsNow(), [workspaceKey(workspace)]: id };
  pins = next;
  saveState(PINS, next);
  changed();
}

/** Where sends go: the workspace's pinned terminal while it's open, else its first opened.
 * Undefined only when it has no terminal. */
export function sendTarget(workspace: WorkspaceName): string | undefined {
  const id = pinsNow()[workspaceKey(workspace)];
  if (id !== undefined && terminals.get(id)?.workspace === workspace) return id;
  let first: string | undefined;
  for (const [t, term] of terminals) {
    if (term.workspace !== workspace) continue;
    if (first === undefined || (opened.get(t) ?? 0) < (opened.get(first) ?? 0)) first = t;
  }
  return first;
}

/** Calls `l` on every change to the terminals, their order or the pins. */
export function subscribeTerminals(l: () => void) {
  listeners.add(l);
  return () => listeners.delete(l);
}

/** Terminal `id`'s tab title, if it's open. */
export const terminalTitle = (id: string) => terminals.get(id)?.title();

/** Brings terminal `id` forward and focuses it, if it's open. */
export const focusTerminal = (id: string) => terminals.get(id)?.focus();

/** Whether terminal `id` is open. */
export const hasTerminal = (id: string) => terminals.has(id);

/** The workspace's send target, kept current. */
export function useSendTarget(workspace: WorkspaceName) {
  useSyncExternalStore(subscribeTerminals, () => version);
  return sendTarget(workspace);
}

export interface TerminalChoice {
  id: string;
  title: string;
  canPaste: boolean;
}

/** The workspace's terminals, the send target first, then most recently focused. */
export function workspaceTerminals(workspace: WorkspaceName): TerminalChoice[] {
  const target = sendTarget(workspace);
  return [...terminals]
    .filter(([, t]) => t.workspace === workspace)
    .reverse()
    .sort(([a], [b]) => Number(b === target) - Number(a === target))
    .map(([id, t]) => ({ id, title: t.title(), canPaste: t.canPaste() }));
}

type SendResult = "sent" | "noTerminal" | "unsafe";

/** Until when the next send also presses Enter. */
let submitUntil = 0;
const SUBMIT_MS = 5000;

/** Has the next send, within a few seconds, also press Enter when an agent runs in its
 * terminal, so the agent starts on it. Buttons call it before working out what to send. */
export function submitNextSend() {
  submitUntil = Date.now() + SUBMIT_MS;
}

/** C0 and C1 control characters other than tab and newline. An escape in pasted text could end
 * bracketed paste early and have the shell run the rest. */
// biome-ignore lint/suspicious/noControlCharactersInRegex: matching them is the point
const CONTROLS = /[\x00-\x08\x0b-\x1f\x7f-\x9f]/g;

/** Pastes `text` into terminal `id`, or else the workspace's send target, and focuses it. Nothing is sent when that terminal's program doesn't use bracketed paste, since
 * a plain shell would run each pasted line. */
export function sendToTerminal(workspace: WorkspaceName, text: string, id?: string): SendResult {
  const to = id ?? sendTarget(workspace);
  const target = to === undefined ? undefined : terminals.get(to);
  const submit = Date.now() < submitUntil;
  submitUntil = 0;
  if (!target) return "noTerminal";
  if (!target.canPaste()) return "unsafe";
  target.paste(text.replace(CONTROLS, ""));
  // A plain shell would run the text, so Enter waits for an agent.
  if (submit && target.hasAgent()) target.submit();
  target.focus();
  return "sent";
}

/** Pastes `text` into terminal `id` and presses Enter, without focusing it. Returns false when
 * no agent runs there or it can't take a paste. */
export function promptAgent(id: string, text: string): boolean {
  const t = terminals.get(id);
  if (!t?.hasAgent() || !t.canPaste()) return false;
  t.paste(text.replace(CONTROLS, ""));
  t.submit();
  t.prompted();
  return true;
}

/** Whether an agent runs in terminal `id`. */
export const terminalHasAgent = (id: string) => !!terminals.get(id)?.hasAgent();

/** Whether terminal `id`'s agent is working or waiting on the user. */
export const terminalAgentBusy = (id: string) => !!terminals.get(id)?.agentBusy();

/** Stops the turn of terminal `id`'s agent. */
export const interruptAgent = (id: string) => terminals.get(id)?.interrupt();
