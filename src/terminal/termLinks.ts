import type { IBufferRange, ILink } from "@xterm/xterm";
import { isMac } from "../core/keys";

const isHeld = (e: KeyboardEvent | MouseEvent) => (isMac ? e.metaKey : e.ctrlKey);

/** The link under the pointer, underlined only while ⌘ (Ctrl elsewhere) is held. */
let hovered: ILink | undefined;
let held = false;

function show() {
  if (!hovered?.decorations) return;
  hovered.decorations.underline = held;
  hovered.decorations.pointerCursor = held;
}

function track(e: KeyboardEvent | MouseEvent) {
  if (isHeld(e) === held) return;
  held = isHeld(e);
  show();
}

let tracking = false;
function startTracking() {
  if (tracking) return;
  tracking = true;
  window.addEventListener("keydown", track, true);
  window.addEventListener("keyup", track, true);
  window.addEventListener("mousemove", track, true);
  window.addEventListener("blur", () => {
    held = false;
    show();
  });
}

/** A terminal link that ⌘-click (Ctrl-click elsewhere) opens, underlined while ⌘ is held. */
export function heldLink(range: IBufferRange, text: string, open: () => void): ILink {
  startTracking();
  const link: ILink = {
    range,
    text,
    decorations: { underline: held, pointerCursor: held },
    activate(e) {
      if (isHeld(e)) open();
    },
    hover(e) {
      held = isHeld(e);
      hovered = link;
      show();
    },
    leave() {
      if (hovered === link) hovered = undefined;
    },
  };
  return link;
}
