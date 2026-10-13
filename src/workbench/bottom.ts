import { useSyncExternalStore } from "react";

/** The bottom panel's state, shared by every workspace and window and remembered. */
export interface BottomState {
  open: boolean;
  height: number;
}

const KEY = "lgtm:bottom";

function load(): BottomState {
  const state = { open: false, height: 260 };
  try {
    const saved = JSON.parse(localStorage.getItem(KEY) ?? "null");
    if (typeof saved?.open === "boolean") state.open = saved.open;
    if (typeof saved?.height === "number") state.height = saved.height;
  } catch {
    // Remembering panel state is a convenience.
  }
  return state;
}

let state = load();
const listeners = new Set<() => void>();
const subscribe = (l: () => void) => {
  listeners.add(l);
  return () => listeners.delete(l);
};

export const bottom = {
  get: () => state,
  set(update: Partial<BottomState>) {
    state = { ...state, ...update };
    try {
      localStorage.setItem(KEY, JSON.stringify(state));
    } catch {
      // Remembering panel state is a convenience.
    }
    for (const l of listeners) l();
  },
  use: () => useSyncExternalStore(subscribe, () => state),
};
