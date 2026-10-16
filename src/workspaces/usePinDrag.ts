import { useRef, useState } from "react";
import { movePin } from "./pins";

/** Pixels the pointer moves before a press becomes a drag. */
const SLOP = 4;

/** Where a dragged pin would land: before the row with `before`, or after the last. */
export interface PinDrop {
  key: string;
  before: string | null;
}

/** Reorders pinned rows by dragging. Pointer events rather than HTML drag and drop, which
 * WebKit drops now and then. Rows carry `data-pin` with their key. */
export function usePinDrag(pinned: string[]) {
  const [drop, setDrop] = useState<PinDrop>();
  const list = useRef(pinned);
  list.current = pinned;

  function start(e: React.PointerEvent, key: string) {
    if (e.button !== 0) return;
    const ul = (e.currentTarget as Element).closest("ul");
    if (!ul) return;
    const [x0, y0] = [e.clientX, e.clientY];
    let dragging = false;
    let at: PinDrop | undefined;

    const move = (m: PointerEvent) => {
      if (!dragging && Math.hypot(m.clientX - x0, m.clientY - y0) < SLOP) return;
      dragging = true;
      const others = [...ul.querySelectorAll<HTMLElement>("[data-pin]")].filter(
        (r) => r.dataset.pin !== key,
      );
      const next = others.find((r) => {
        const b = r.getBoundingClientRect();
        return m.clientY < b.top + b.height / 2;
      });
      at = { key, before: next?.dataset.pin ?? null };
      setDrop(at);
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", up);
      setDrop(undefined);
      if (!dragging) return;
      // The release would otherwise click the row it started on.
      const swallow = (c: MouseEvent) => c.stopPropagation();
      window.addEventListener("click", swallow, { capture: true, once: true });
      setTimeout(() => window.removeEventListener("click", swallow, { capture: true }));
      if (!at) return;
      const rest = list.current.filter((k) => k !== key);
      const i = at.before === null ? rest.length : rest.indexOf(at.before);
      movePin(key, i < 0 ? rest.length : i);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", up);
  }

  return { drop, start };
}
