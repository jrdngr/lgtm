import { type ReactNode, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import "./tip.css";

const DELAY_MS = 250;

/** A label shown beside its element soon after the pointer rests on it: native `title`
 * tooltips wait over a second. `side` is where the label sits. */
export function useTip(label: string, side: "left" | "right") {
  const [at, setAt] = useState<{ x: number; y: number }>();
  const timer = useRef<number>(undefined);
  const hide = () => {
    window.clearTimeout(timer.current);
    setAt(undefined);
  };
  useEffect(() => () => window.clearTimeout(timer.current), []);

  const handlers = {
    onPointerEnter(e: React.PointerEvent<HTMLElement>) {
      const r = e.currentTarget.getBoundingClientRect();
      window.clearTimeout(timer.current);
      timer.current = window.setTimeout(
        () => setAt({ x: side === "left" ? r.left - 6 : r.right + 6, y: r.top + r.height / 2 }),
        DELAY_MS,
      );
    },
    onPointerLeave: hide,
    onPointerDown: hide,
  };
  const tip: ReactNode =
    at &&
    createPortal(
      <div className={`tip tip-${side}`} style={{ left: at.x, top: at.y }}>
        {label}
      </div>,
      document.body,
    );
  return { handlers, tip };
}
