import { useSyncExternalStore } from "react";

/** A value kept in localStorage that components can subscribe to. Falls back to `initial`
 * when nothing valid is saved. */
export function remembered<T extends string | boolean>(key: string, initial: T, valid: T[]) {
  let value = initial;
  try {
    const saved = JSON.parse(localStorage.getItem(key) ?? "null");
    if (valid.includes(saved)) value = saved;
  } catch {
    // Remembering is a convenience.
  }
  const listeners = new Set<() => void>();
  const subscribe = (l: () => void) => {
    listeners.add(l);
    return () => listeners.delete(l);
  };
  return {
    get: () => value,
    set(next: T) {
      value = next;
      try {
        localStorage.setItem(key, JSON.stringify(next));
      } catch {
        // Remembering is a convenience.
      }
      for (const l of listeners) l();
    },
    use: () => useSyncExternalStore(subscribe, () => value),
  };
}
