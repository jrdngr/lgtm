import { useSyncExternalStore } from "react";
import { flushSync } from "react-dom";
import { bottom } from "../core/bottom";
import { bottomDockOf, dockReady } from "../core/docks";
import { addClaudeBehind } from "../core/panels";
import { type WorkspaceName, workspaceKey } from "../core/workspace";
import { loadedSettings } from "../settings/store";
import {
  keepSubmit,
  pinTarget,
  sendTarget,
  sendToTerminal,
  type TerminalChoice,
  terminalReady,
  workspaceTerminals,
} from "./registry";

/** The workspace's terminals offered while no background terminal is pinned. */
export interface BackgroundAsk {
  terminals: TerminalChoice[];
  /** The terminal picked, or undefined when the pick was dismissed. */
  answer(id: string | undefined): void;
}

let asking: BackgroundAsk | undefined;
const listeners = new Set<() => void>();
const set = (a: BackgroundAsk | undefined) => {
  asking = a;
  for (const l of listeners) l();
};

export function useBackgroundAsk(): BackgroundAsk | undefined {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => asking,
  );
}

/** How long a new background terminal's agent gets to start. */
const START_MS = 60_000;
/** How long the bottom panel gets to come up. */
const BOTTOM_MS = 2_000;
/** Background terminals starting, by workspace key, so sends meanwhile wait for the same one. */
const starting = new Map<string, Promise<string | undefined>>();

/** Opens a Claude tab named `name` behind the focused one, pins it for background sends, and
 * resolves to it once its agent can take a prompt, or once it's had time to start. */
/** The workspace's bottom dock, shown, or its main dock when the bottom one doesn't come up. */
async function bottomDock(ws: WorkspaceName) {
  if (!bottom.get().open) flushSync(() => bottom.set({ open: true }));
  const until = Date.now() + BOTTOM_MS;
  while (!bottomDockOf(ws) && Date.now() < until) {
    await new Promise((r) => setTimeout(r, 50));
  }
  return bottomDockOf(ws) ?? dockReady(ws);
}

async function openBackground(ws: WorkspaceName, name: string): Promise<string> {
  const id = addClaudeBehind(await bottomDock(ws), name).id;
  pinTarget(ws, id, "background");
  const until = Date.now() + START_MS;
  while (!terminalReady(id) && Date.now() < until) {
    await new Promise((r) => setTimeout(r, 250));
  }
  return id;
}

/** A new background terminal when the settings say to open one, else undefined. */
async function autoOpen(ws: WorkspaceName): Promise<string | undefined> {
  const settings = await loadedSettings();
  return settings.backgroundAutoOpen ? openBackground(ws, settings.backgroundName) : undefined;
}

/** The workspace's background terminal. With none pinned, opens one when the settings say
 * to, else asks which to use and pins it. Undefined when it has no terminal or the pick is
 * dismissed. */
export async function backgroundTarget(ws: WorkspaceName): Promise<string | undefined> {
  const key = workspaceKey(ws);
  const pinned = starting.has(key) ? undefined : sendTarget(ws, "background");
  if (pinned) return pinned;
  let open = starting.get(key);
  if (!open) {
    open = autoOpen(ws).finally(() => starting.delete(key));
    starting.set(key, open);
  }
  const opened = await open;
  if (opened) return opened;
  const terminals = workspaceTerminals(ws, "background");
  if (!terminals.length) return undefined;
  asking?.answer(undefined);
  return new Promise((resolve) =>
    set({
      terminals,
      answer(id) {
        set(undefined);
        if (id) pinTarget(ws, id, "background");
        resolve(id);
      },
    }),
  );
}

/** Pastes `text` into the workspace's background terminal without focusing it. Resolves with
 * the terminal it went to, or why it didn't go: an error, or nothing when the pick was
 * dismissed. */
export async function sendBackground(
  ws: WorkspaceName,
  text: string,
): Promise<{ terminal: string } | { error?: string }> {
  const id = await keepSubmit(backgroundTarget(ws));
  if (!id && workspaceTerminals(ws).length) return {};
  const result = sendToTerminal(ws, text, id, "background");
  if (result === "sent" && id) return { terminal: id };
  return {
    error:
      result === "unsafe"
        ? "That terminal can't take a paste safely: its program would run each line. Start your agent there first."
        : "No terminal is open. Open one with ⌘T and start your agent.",
  };
}
