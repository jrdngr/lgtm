/** Tells views that read files the editor writes, like persona files, that one was saved. */
const listeners = new Set<(path: string) => void>();

export function onSaved(l: (path: string) => void) {
  listeners.add(l);
  return () => {
    listeners.delete(l);
  };
}

export function emitSaved(path: string) {
  for (const l of listeners) l(path);
}
