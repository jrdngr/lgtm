import { type ReactNode, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

/** How long the pointer rests on a row before its popover shows. */
const DELAY_MS = 500;

interface Hover {
  row: number;
  rect: DOMRect;
}

/** The row the pointer rests on, once it has rested a moment. Anything else the pointer does
 * hides it. */
export function useRowHover() {
  const [hover, setHover] = useState<Hover>();
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);

  function leave() {
    clearTimeout(timer.current);
    setHover(undefined);
  }

  function enter(row: number, el: HTMLElement) {
    leave();
    timer.current = setTimeout(() => setHover({ row, rect: el.getBoundingClientRect() }), DELAY_MS);
  }

  useEffect(() => () => clearTimeout(timer.current), []);
  return { hover, enter, leave };
}

/** A revision's row again over it, wider and with nothing cut short. */
export function RevisionPopover({ hover, children }: { hover: Hover; children: ReactNode }) {
  const { left, top, width } = hover.rect;
  return createPortal(
    <div
      className="lr lr-pop"
      style={{ left, top, minWidth: width, maxWidth: `min(760px, calc(100vw - ${left + 16}px))` }}
    >
      {children}
    </div>,
    document.body,
  );
}
