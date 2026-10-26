import { useSyncExternalStore } from "react";
import type { WorkspaceName } from "../core/workspace";
import {
  keepSubmit,
  pinTarget,
  sendTarget,
  sendToTerminal,
  type TerminalChoice,
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

/** The workspace's background terminal. With none pinned, asks which to use and pins it.
 * Undefined when it has no terminal or the pick is dismissed. */
export function backgroundTarget(ws: WorkspaceName): Promise<string | undefined> {
  const pinned = sendTarget(ws, "background");
  if (pinned) return Promise.resolve(pinned);
  const terminals = workspaceTerminals(ws, "background");
  if (!terminals.length) return Promise.resolve(undefined);
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
