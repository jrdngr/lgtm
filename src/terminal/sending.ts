import { useEffect, useReducer, useSyncExternalStore } from "react";
import { subscribeActivity, tabAgent } from "../workspaces/activity";

/** Which violet button each background send came from, so the button can spin while the
 * terminal's agent works on it. A click claims the next background send for its button. */

/** How long a click waits for its send, which may wait on the pick of a terminal. */
const CLAIM_MS = 60_000;
/** How long a send counts as being worked on before the agent's hooks report on it. */
const LAND_MS = 15_000;

let claim: { key: string; until: number } | undefined;
/** Sends by button key: the terminal, when, and whether its agent has been seen working. */
const sends = new Map<string, { terminal: string; at: number; seen: boolean }>();
const listeners = new Set<() => void>();
const changed = () => {
  for (const l of listeners) l();
};

/** Has the next background send count as button `key`'s. */
export function claimSend(key: string) {
  claim = { key, until: Date.now() + CLAIM_MS };
}

/** A prompt went to background terminal `terminal`. */
export function sentInBackground(terminal: string) {
  if (!claim || Date.now() > claim.until) return;
  sends.set(claim.key, { terminal, at: Date.now(), seen: false });
  claim = undefined;
  changed();
}

/** Whether button `key`'s send is still being worked on. */
export function sending(key: string, now = Date.now()): boolean {
  const s = sends.get(key);
  if (!s) return false;
  const state = tabAgent(s.terminal)?.state;
  if (state === "working" || state === "input" || state === "stalled") {
    s.seen = true;
    return true;
  }
  if (!s.seen && now - s.at < LAND_MS) return true;
  sends.delete(key);
  return false;
}

function subscribe(l: () => void) {
  listeners.add(l);
  const off = subscribeActivity(l);
  return () => {
    listeners.delete(l);
    off();
  };
}

/** Whether button `key`'s send is still being worked on, kept current. */
export function useSending(key: string): boolean {
  const on = useSyncExternalStore(subscribe, () => sending(key));
  // A send no hook reports on stops counting after a while, which no event marks.
  const [, tick] = useReducer((n: number) => n + 1, 0);
  useEffect(() => {
    if (!on) return;
    const t = window.setInterval(tick, 1000);
    return () => window.clearInterval(t);
  }, [on]);
  return on;
}
