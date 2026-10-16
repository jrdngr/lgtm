import { useSyncExternalStore } from "react";
import { loadState, saveState } from "../workbench/savedState";
import { type WorkspaceName, workspaceKey } from "../workspaces/context";

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
  changed();
  return () => {
    terminals.delete(id);
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

/** Pins terminal `id` as the workspace's send target, or with `null`, unpins it. */
export function pinTarget(workspace: WorkspaceName, id: string | null) {
  const next = { ...pinsNow() };
  if (id === null) delete next[workspaceKey(workspace)];
  else next[workspaceKey(workspace)] = id;
  pins = next;
  saveState(PINS, next);
  changed();
}

/** The workspace's pinned terminal, while it's open. */
export function pinnedTarget(workspace: WorkspaceName): string | undefined {
  const id = pinsNow()[workspaceKey(workspace)];
  return id !== undefined && terminals.get(id)?.workspace === workspace ? id : undefined;
}

/** Where sends go: the pinned terminal, else the most recently focused. */
export function sendTarget(workspace: WorkspaceName): string | undefined {
  return (
    pinnedTarget(workspace) ??
    [...terminals].reverse().find(([, t]) => t.workspace === workspace)?.[0]
  );
}

function subscribe(l: () => void) {
  listeners.add(l);
  return () => listeners.delete(l);
}

/** The workspace's send target and whether it's pinned, kept current. */
export function useSendTarget(workspace: WorkspaceName) {
  useSyncExternalStore(subscribe, () => version);
  return { target: sendTarget(workspace), pinned: pinnedTarget(workspace) };
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

/** C0 and C1 control characters other than tab and newline. An escape in pasted text could end
 * bracketed paste early and have the shell run the rest. */
// biome-ignore lint/suspicious/noControlCharactersInRegex: matching them is the point
const CONTROLS = /[\x00-\x08\x0b-\x1f\x7f-\x9f]/g;

/** Pastes `text` into terminal `id`, or else the workspace's send target, and focuses it. Nothing is sent when that terminal's program doesn't use bracketed paste, since
 * a plain shell would run each pasted line. */
export function sendToTerminal(workspace: WorkspaceName, text: string, id?: string): SendResult {
  const to = id ?? sendTarget(workspace);
  const target = to === undefined ? undefined : terminals.get(to);
  if (!target) return "noTerminal";
  if (!target.canPaste()) return "unsafe";
  target.paste(text.replace(CONTROLS, ""));
  target.focus();
  return "sent";
}
