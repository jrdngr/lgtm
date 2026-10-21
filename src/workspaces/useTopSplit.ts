import { useRef, useState } from "react";
import { loadState, saveState } from "../core/savedState";

const KEY = "workspacesHeight";
/** Room the revisions keep below a dragged split, and the least the workspaces get. */
const MIN_REVISIONS = 120;
const MIN_TOP = 40;

const isHeight = (v: unknown): v is number | null => v === null || typeof v === "number";

/** The height of the workspaces above the revisions: their own, up to a cap, until the split
 * between them is dragged. A double-click on the split goes back to that. */
export function useTopSplit() {
  const [height, setHeight] = useState(() => loadState(KEY, null, isHeight));
  const top = useRef<HTMLDivElement>(null);

  function set(h: number | null) {
    setHeight(h);
    saveState(KEY, h);
  }

  function onPointerDown(e: React.PointerEvent) {
    const el = top.current;
    const box = el?.parentElement;
    if (!el || !box || e.button !== 0) return;
    e.preventDefault();
    const startY = e.clientY;
    const start = el.offsetHeight;
    const max = box.clientHeight - MIN_REVISIONS;
    let h = start;
    const move = (m: PointerEvent) => {
      h = Math.round(Math.min(Math.max(start + m.clientY - startY, MIN_TOP), max));
      setHeight(h);
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      set(h);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }

  return {
    top,
    style: height === null ? undefined : { height, maxHeight: "none" },
    splitter: { onPointerDown, onDoubleClick: () => set(null) },
  };
}
