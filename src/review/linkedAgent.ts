import { useSyncExternalStore } from "react";
import type { TerminalAgent } from "../bindings";
import { type WorkspaceName, workspaceKey } from "../core/workspace";
import { commands } from "../ipc";
import { watchAgentHooks } from "../terminal/agentHooks";
import { hasTerminal, subscribeTerminals, terminalTitle } from "../terminal/registry";
import { threadContext } from "./agentText";
import { currentReview, onReviewChange } from "./current";

/** A terminal whose agent is linked to a workspace's PR review. */
interface Link {
  terminal: string;
  /** How its agent works on the PR's threads, which lgtm's hook gives it once. */
  brief: string;
  working: boolean;
}

/** By workspace key. */
const links = new Map<string, Link>();
/** The thread I'm on in each workspace, by workspace key. */
const current = new Map<string, number>();
/** Where a terminal's last prompt was filed, so its answer goes with it. */
const filed = new Map<string, { review: number; thread: number } | null>();
/** Prompts lgtm just pasted, by terminal: not filed as something I said, with the thread the
 * answer goes to, if any. */
const pasted = new Map<string, { thread: number | null; at: number }>();
/** How long a paste waits for its prompt to be sent. */
const PASTE_MS = 10_000;
/** Threads whose chat is open. */
const openChats = new Set<number>();
/** What each linked terminal was last told, so unchanged context isn't sent again. */
const pushed = new Map<string, string>();

const listeners = new Set<() => void>();
let version = 0;
function changed() {
  version++;
  for (const l of listeners) l();
}

const workspaceOf = (terminal: string) => [...links].find(([, l]) => l.terminal === terminal)?.[0];

/** Tells the backend the brief and the thread I'm on, for lgtm's hook to add to the agent's
 * prompts. */
function push(key: string) {
  const link = links.get(key);
  if (!link) return;
  const review = currentReview(key || null);
  const id = current.get(key);
  const t = review?.threads.find((t) => t.id === id);
  const thread = t
    ? threadContext(
        review?.files ?? [],
        t,
        review?.comments.find((c) => c.id === t.id),
      )
    : null;
  const said = JSON.stringify([link.brief, thread]);
  if (pushed.get(link.terminal) === said) return;
  pushed.set(link.terminal, said);
  commands.setAgentLink(link.terminal, true, link.brief, thread);
}

function drop(key: string) {
  const link = links.get(key);
  if (!link) return;
  links.delete(key);
  pushed.delete(link.terminal);
  commands.setAgentLink(link.terminal, false, null, null);
  changed();
}

/** Files a hook report from a linked terminal in the thread I was on when I sent the prompt. */
export function mirror({ terminal, state, prompt, reply }: TerminalAgent) {
  const key = workspaceOf(terminal);
  if (key === undefined) return;
  const link = links.get(key);
  if (!link) return;
  if (prompt) {
    const review = currentReview(key || null);
    const paste = pasted.get(terminal);
    pasted.delete(terminal);
    const ours = paste && Date.now() - paste.at < PASTE_MS;
    // A paste's answer goes to its thread.
    const thread = ours ? paste.thread : current.get(key);
    const to = review && thread != null ? { review: review.id, thread } : null;
    filed.set(terminal, to);
    if (to && !ours) commands.addChatMessage(to.review, to.thread, "user", prompt);
  }
  const to = filed.get(terminal);
  if (reply && to) commands.addChatMessage(to.review, to.thread, "agent", reply);
  if (state === "gone") return drop(key);
  const working = state === "working";
  if (working !== link.working) {
    links.set(key, { ...link, working });
    changed();
  }
}

let watching = false;
function watch() {
  if (watching) return;
  watching = true;
  watchAgentHooks(mirror);
  subscribeTerminals(() => {
    for (const [key, l] of links) {
      if (!hasTerminal(l.terminal)) drop(key);
    }
    changed();
  });
  // The thread's text follows edits to it.
  onReviewChange(() => {
    for (const key of links.keys()) push(key);
  });
}

/** Links `terminal`'s agent to the workspace's PR review with `brief`, unlinking any other
 * terminal. */
export function linkAgent(workspace: WorkspaceName, terminal: string, brief: string) {
  watch();
  const key = workspaceKey(workspace);
  if (links.get(key)?.terminal !== terminal) drop(key);
  // A terminal is linked to one review at a time.
  for (const [other, l] of links) if (l.terminal === terminal && other !== key) links.delete(other);
  links.set(key, { terminal, brief, working: false });
  push(key);
  changed();
}

export function unlinkAgent(workspace: WorkspaceName) {
  drop(workspaceKey(workspace));
}

/** The terminal linked to the workspace's review, outside React. */
export const linkedTerminal = (workspace: WorkspaceName) =>
  links.get(workspaceKey(workspace))?.terminal;

/** Marks the next prompt from `terminal` as one lgtm pasted, so it isn't filed as something I
 * said, and its answer goes to `thread`, or nowhere. */
export function notePasted(terminal: string, thread: number | null) {
  pasted.set(terminal, { thread, at: Date.now() });
}

/** Makes thread `id` the one I'm on in the workspace. */
export function setCurrentThread(workspace: WorkspaceName, id: number) {
  const key = workspaceKey(workspace);
  if (current.get(key) === id) return;
  current.set(key, id);
  push(key);
  changed();
}

/** Opens or closes thread `id`'s chat. */
export function setChatOpen(id: number, open: boolean) {
  if (open === openChats.has(id)) return;
  if (open) openChats.add(id);
  else openChats.delete(id);
  changed();
}

function subscribe(l: () => void) {
  listeners.add(l);
  return () => listeners.delete(l);
}

/** The workspace's linked terminal, its title and whether its agent is working, and the
 * thread I'm on. */
export function useLink(workspace: WorkspaceName) {
  useSyncExternalStore(subscribe, () => version);
  const key = workspaceKey(workspace);
  const link = links.get(key);
  return {
    terminal: link?.terminal,
    title: link ? (terminalTitle(link.terminal) ?? "") : "",
    working: !!link?.working,
    current: current.get(key),
  };
}

/** Whether thread `id`'s chat is open. */
export function useChatOpen(id: number) {
  useSyncExternalStore(subscribe, () => version);
  return openChats.has(id);
}
