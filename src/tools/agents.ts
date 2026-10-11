import { useSyncExternalStore } from "react";

/** Whether `claude` is missing or too old and the user chose to go on without it. Agent
 * features (persona reviews, comment workshopping) are off then. */
let off = false;
const listeners = new Set<() => void>();

export function setAgentsOff(value: boolean) {
  off = value;
  for (const l of listeners) l();
}

function subscribe(l: () => void) {
  listeners.add(l);
  return () => listeners.delete(l);
}

export const agentsOff = () => off;

export const useAgentsOff = () => useSyncExternalStore(subscribe, () => off);
