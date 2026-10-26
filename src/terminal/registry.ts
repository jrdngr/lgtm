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

/** What a send is for: a prompt I'll answer in the terminal, or one whose agent reports back to
 * lgtm. Each has its own pinned terminal per workspace. */
export type Role = "interactive" | "background";

/** Each workspace's pinned terminal for a role, by workspace key. Saved per repo. */
type Pins = Record<string, string>;
const PINS: Record<Role, string> = { interactive: "send-pins", background: "send-pins-bg" };
const isPins = (v: unknown): v is Pins =>
  typeof v === "object" && v !== null && Object.values(v).every((id) => typeof id === "string");
const pins: Partial<Record<Role, Pins>> = {};
const pinsNow = (role: Role) => {
  pins[role] ??= loadState(PINS[role], {}, isPins);
  return pins[role];
};

function savePins(role: Role, next: Pins) {
  pins[role] = next;
  saveState(PINS[role], next);
}

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

/** Pins terminal `id` as the workspace's terminal for `role`, taking its other pin off it, or
 * unpins it when it already is. */
export function pinTarget(workspace: WorkspaceName, id: string, role: Role) {
  const key = workspaceKey(workspace);
  const other: Role = role === "interactive" ? "background" : "interactive";
  const { [key]: was, ...rest } = pinsNow(role);
  savePins(role, was === id ? rest : { ...rest, [key]: id });
  if (pinsNow(other)[key] === id) {
    const { [key]: _, ...others } = pinsNow(other);
    savePins(other, others);
  }
  changed();
}

/** Terminal `id` of the workspace that's pinned for `role`, while it's open. */
function pinned(workspace: WorkspaceName, role: Role): string | undefined {
  const id = pinsNow(role)[workspaceKey(workspace)];
  return id !== undefined && terminals.get(id)?.workspace === workspace ? id : undefined;
}

/** Where sends for `role` go: the workspace's pinned terminal for it while it's open. Else
 * interactive sends go to its first opened terminal, other than the background one, and
 * background sends go nowhere until one is pinned. */
export function sendTarget(
  workspace: WorkspaceName,
  role: Role = "interactive",
): string | undefined {
  const id = pinned(workspace, role);
  if (id !== undefined || role === "background") return id;
  const background = pinned(workspace, "background");
  let first: string | undefined;
  for (const [t, term] of terminals) {
    if (term.workspace !== workspace || t === background) continue;
    if (first === undefined || (opened.get(t) ?? 0) < (opened.get(first) ?? 0)) first = t;
  }
  return first;
}

/** The role terminal `id` is pinned for in its workspace, if any. */
export function roleOf(workspace: WorkspaceName, id: string): Role | undefined {
  if (pinned(workspace, "background") === id) return "background";
  if (sendTarget(workspace) === id) return "interactive";
  return undefined;
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

/** Each terminal's role in the workspace, kept current. */
export function useRoles(workspace: WorkspaceName) {
  useSyncExternalStore(subscribeTerminals, () => version);
  return (id: string) => roleOf(workspace, id);
}

export interface TerminalChoice {
  id: string;
  title: string;
  canPaste: boolean;
}

/** The workspace's terminals, the target for `role` first, then most recently focused. */
export function workspaceTerminals(
  workspace: WorkspaceName,
  role: Role = "interactive",
): TerminalChoice[] {
  const target = sendTarget(workspace, role);
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

/** `text`, with a send armed by `submitNextSend` before it kept armed until it resolves, for
 * text that takes longer to work out than the window. */
export async function keepSubmit<T>(text: Promise<T>): Promise<T> {
  const armed = Date.now() < submitUntil;
  const out = await text;
  if (armed) submitNextSend();
  return out;
}

/** C0 and C1 control characters other than tab and newline. An escape in pasted text could end
 * bracketed paste early and have the shell run the rest. */
// biome-ignore lint/suspicious/noControlCharactersInRegex: matching them is the point
const CONTROLS = /[\x00-\x08\x0b-\x1f\x7f-\x9f]/g;

/** Pastes `text` into terminal `id`, or else the workspace's target for `role`, focusing it
 * for an interactive send. Nothing is sent when that terminal's program doesn't use bracketed
 * paste, since a plain shell would run each pasted line. */
export function sendToTerminal(
  workspace: WorkspaceName,
  text: string,
  id?: string,
  role: Role = "interactive",
): SendResult {
  const to = id ?? sendTarget(workspace, role);
  const target = to === undefined ? undefined : terminals.get(to);
  const submit = Date.now() < submitUntil;
  submitUntil = 0;
  if (!target) return "noTerminal";
  if (!target.canPaste()) return "unsafe";
  target.paste(text.replace(CONTROLS, ""));
  // A plain shell would run the text, so Enter waits for an agent.
  if (submit && target.hasAgent()) target.submit();
  if (role === "interactive") target.focus();
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
