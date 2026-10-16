import { isMac } from "../workbench/keys";

let tracking = false;

/** Keeps `cmd-held` on the root element while ⌘ (Ctrl elsewhere) is down, so links can show
 * they open on a click. */
export function trackCmd() {
  if (tracking) return;
  tracking = true;
  const root = document.documentElement;
  const sync = (e: KeyboardEvent | MouseEvent) =>
    root.classList.toggle("cmd-held", isMac ? e.metaKey : e.ctrlKey);
  window.addEventListener("keydown", sync, true);
  window.addEventListener("keyup", sync, true);
  window.addEventListener("mousemove", sync, true);
  window.addEventListener("blur", () => root.classList.remove("cmd-held"));
}

/** Whether a click was made with ⌘ (Ctrl elsewhere) held. */
export const cmdClick = (e: { metaKey: boolean; ctrlKey: boolean }) =>
  isMac ? e.metaKey : e.ctrlKey;
