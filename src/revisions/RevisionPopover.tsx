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
  /** The row shown or waiting to show. */
  const current = useRef<number>(undefined);

  function leave() {
    clearTimeout(timer.current);
    current.current = undefined;
    setHover(undefined);
  }

  function enter(row: number, el: HTMLElement) {
    if (current.current === row) return;
    leave();
    current.current = row;
    timer.current = setTimeout(() => setHover({ row, rect: el.getBoundingClientRect() }), DELAY_MS);
  }

  useEffect(() => () => clearTimeout(timer.current), []);
  return { hover, enter, leave };
}

/** A revision's row again over it, as tall but wider, so its line and bookmarks aren't cut short. */
export function RevisionPopover({ hover, children }: { hover: Hover; children: ReactNode }) {
  const { left, top, width, height } = hover.rect;
  return createPortal(
    <div
      className="lr lr-pop"
      style={{ left, top, height, minWidth: width, maxWidth: `calc(100vw - ${left + 16}px)` }}
    >
      {children}
    </div>,
    document.body,
  );
}
