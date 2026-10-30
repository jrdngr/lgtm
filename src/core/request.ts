/** How long a request waits for a listener before it's dropped. */
const HOLD_MS = 1000;

/** A request to a panel that may still be opening: listeners get it now, or, when none is
 * mounted yet, the first one to mount within a second gets it. */
export function requests<T>() {
  const listeners = new Set<(value: T) => void>();
  let held: { value: T; until: number } | undefined;
  return {
    send(value: T) {
      held = undefined;
      if (listeners.size === 0) held = { value, until: Date.now() + HOLD_MS };
      for (const l of listeners) l(value);
    },
    listen(f: (value: T) => void) {
      listeners.add(f);
      const waiting = held;
      held = undefined;
      if (waiting && Date.now() <= waiting.until) f(waiting.value);
      return () => {
        listeners.delete(f);
      };
    },
  };
}
