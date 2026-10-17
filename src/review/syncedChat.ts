import { useSyncExternalStore } from "react";
import type { TerminalAgent } from "../bindings";
import { commands } from "../ipc";
import { watchAgentHooks } from "../terminal/agentHooks";
import { hasTerminal, subscribeTerminals } from "../terminal/registry";

/** The one thread whose chat a terminal mirrors: what the user submits there and what the
 * agent answers land in the thread. */
interface Sync {
  terminal: string;
  review: number;
  comment: number;
  working: boolean;
}

let sync: Sync | undefined;
/** The last thread synced and its terminal, so Resume can pick the terminal up again. */
let last: Pick<Sync, "terminal" | "comment"> | undefined;
const listeners = new Set<() => void>();

function set(next: Sync | undefined) {
  const prev = sync;
  sync = next;
  if (prev && prev.comment !== next?.comment) commands.allowSuggestions(prev.comment, false);
  if (next && next.comment !== prev?.comment) commands.allowSuggestions(next.comment, true);
  if (next) last = { terminal: next.terminal, comment: next.comment };
  for (const l of listeners) l();
}

/** The marker in the text that starts a sync, which isn't mirrored back. */
export const syncMarker = (comment: number) => `lgtm thread ${comment}`;

/** Mirrors a hook report from the synced terminal into its thread. */
export function mirror({ terminal, state, prompt, reply }: TerminalAgent) {
  const s = sync;
  if (!s || s.terminal !== terminal) return;
  const add = (author: "user" | "agent", text: string) =>
    commands.addChatMessage(s.review, s.comment, author, text).then((r) => {
      if (r.status === "error" && sync === s) set(undefined);
    });
  if (prompt && !prompt.includes(syncMarker(s.comment))) add("user", prompt);
  if (reply) add("agent", reply);
  if (state === "gone") return set(undefined);
  const working = state === "working";
  if (working !== s.working) set({ ...s, working });
}

let watching = false;
function watch() {
  if (watching) return;
  watching = true;
  watchAgentHooks(mirror);
  subscribeTerminals(() => {
    if (sync && !hasTerminal(sync.terminal)) set(undefined);
  });
}

/** Syncs comment `comment`'s chat with terminal `terminal`, ending any other sync. */
export function startSync(terminal: string, review: number, comment: number) {
  watch();
  set({ terminal, review, comment, working: false });
}

export function stopSync() {
  set(undefined);
}

/** Whether comment `comment` was last synced with `terminal`, so syncing it there again
 * needs no new start text. */
export function resumable(comment: number, terminal: string | undefined): boolean {
  return last?.comment === comment && last.terminal === terminal && hasTerminal(terminal);
}

function subscribe(l: () => void) {
  listeners.add(l);
  return () => listeners.delete(l);
}

/** Whether comment `comment`'s chat is synced, and whether its agent is working. */
export function useSyncedChat(comment: number | undefined) {
  const s = useSyncExternalStore(subscribe, () => sync);
  const synced = comment !== undefined && s?.comment === comment;
  return { synced, working: synced && !!s?.working };
}
