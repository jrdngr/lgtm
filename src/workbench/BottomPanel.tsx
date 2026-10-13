import { useRef } from "react";
import { bottom } from "./bottom";

const MIN_HEIGHT = 80;
/** Room left above the panel for the toolbar and some of the panes. */
const MIN_ABOVE = 160;

/** A panel docked along the bottom of the window, under the side panels and panes. Each
 * workspace's bottom dock renders into `docksRef`. Hidden rather than unmounted, so its
 * terminals keep running. */
export function BottomPanel({ docksRef }: { docksRef(el: HTMLDivElement | null): void }) {
  const { open, height } = bottom.use();
  const drag = useRef<{ y: number; height: number }>(undefined);

  function startResize(e: React.PointerEvent) {
    e.preventDefault();
    drag.current = { y: e.clientY, height };
    (e.target as Element).setPointerCapture(e.pointerId);
  }

  function resize(e: React.PointerEvent) {
    if (!drag.current) return;
    const next = drag.current.height + drag.current.y - e.clientY;
    const max = window.innerHeight - MIN_ABOVE;
    bottom.set({ height: Math.round(Math.max(MIN_HEIGHT, Math.min(max, next))) });
  }

  return (
    <div className="bottom-panel" hidden={!open} style={{ height }}>
      <div className="docks" ref={docksRef} />
      <div
        className="bottom-resizer"
        onPointerDown={startResize}
        onPointerMove={resize}
        onPointerUp={() => {
          drag.current = undefined;
        }}
      />
    </div>
  );
}
