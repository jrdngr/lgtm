export const isMac = navigator.platform.startsWith("Mac");

/**
 * Normalizes an app shortcut to a string like "Mod+Shift+D" or "Mod+Alt+B". Returns undefined
 * for keys that aren't app chords, so they reach the focused panel. Mod is ⌘ on macOS and
 * Ctrl+Shift elsewhere, because plain Ctrl chords (Ctrl+D, Ctrl+T) belong to the shell.
 */
export function chord(e: KeyboardEvent): string | undefined {
  const mod = isMac ? e.metaKey && !e.ctrlKey : e.ctrlKey && e.shiftKey;
  if (!mod) return undefined;
  const shift = isMac && e.shiftKey ? "Shift+" : "";
  const alt = e.altKey ? "Alt+" : "";
  // Option changes the character macOS reports (⌥B is "∫"), so those letters come from the
  // key code. Other chords use the character, which follows the keyboard layout.
  const key = e.altKey && /^Key[A-Z]$/.test(e.code) ? e.code.slice(3) : e.key.toUpperCase();
  return `Mod+${alt}${shift}${key}`;
}

/** Ctrl+` toggles the bottom panel and Ctrl+Shift+` opens a terminal in it, on every platform,
 * as in VS Code. By key code, so it's the key left of 1 on any layout. */
export function bottomKey(e: KeyboardEvent): "toggle" | "terminal" | undefined {
  if (e.code !== "Backquote" || !e.ctrlKey || e.metaKey || e.altKey) return undefined;
  return e.shiftKey ? "terminal" : "toggle";
}
