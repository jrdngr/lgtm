import { useSyncExternalStore } from "react";

/** The panel whose tab is being renamed in place, if any. */
let renaming: string | null = null;
const listeners = new Set<() => void>();

export function setRenaming(id: string | null) {
  renaming = id;
  for (const l of listeners) l();
}

const subscribe = (l: () => void) => {
  listeners.add(l);
  return () => listeners.delete(l);
};

/** Whether panel `id`'s tab is being renamed. */
export const useRenaming = (id: string) => useSyncExternalStore(subscribe, () => renaming === id);
