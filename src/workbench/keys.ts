const isMac = navigator.platform.startsWith("Mac");

/**
 * Normalizes an app shortcut to a string like "Mod+Shift+D". Returns undefined for keys that
 * aren't app chords, so they reach the focused panel. Mod is ⌘ on macOS and Ctrl+Shift
 * elsewhere, because plain Ctrl chords (Ctrl+D, Ctrl+T) belong to the shell.
 */
export function chord(e: KeyboardEvent): string | undefined {
  const mod = isMac ? e.metaKey && !e.ctrlKey : e.ctrlKey && e.shiftKey;
  if (!mod || e.altKey) return undefined;
  const shift = isMac && e.shiftKey ? "Shift+" : "";
  return `Mod+${shift}${e.key.toUpperCase()}`;
}
