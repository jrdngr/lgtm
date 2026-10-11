import { useSyncExternalStore } from "react";

/** A boolean outside React that components can subscribe to. */
export function flag() {
  let value = false;
  const listeners = new Set<() => void>();
  const subscribe = (l: () => void) => {
    listeners.add(l);
    return () => listeners.delete(l);
  };
  return {
    set(next: boolean) {
      value = next;
      for (const l of listeners) l();
    },
    get: () => value,
    use: () => useSyncExternalStore(subscribe, () => value),
  };
}
