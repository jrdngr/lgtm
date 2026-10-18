import { useRef } from "react";
import { bottom } from "../core/bottom";
import type { WorkspaceName } from "../core/workspace";
import { BottomStrip, useBottomDock } from "./BottomStrip";

const MIN_HEIGHT = 80;
/** Room left above the panel for the toolbar and some of the panes. */
const MIN_ABOVE = 160;

/** A panel docked under the panes, between the side panels. Each workspace's bottom dock
 * renders into `docksRef`. Collapsed, it's a strip of the selected workspace's tabs, and
 * gone when it has none. Hidden rather than unmounted, so its terminals keep running. */
export function BottomPanel({
  docksRef,
  workspace,
}: {
  docksRef(el: HTMLDivElement | null): void;
  workspace: WorkspaceName;
}) {
  const { open, height } = bottom.use();
  const dock = useBottomDock(workspace);
  const strip = !open && !!dock?.panels.length;
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
    <div className="bottom-panel" hidden={!open && !strip} style={open ? { height } : undefined}>
      <div className="docks" ref={docksRef} hidden={!open} />
      {strip && dock && <BottomStrip api={dock} workspace={workspace} />}
      {open && (
        <div
          className="bottom-resizer"
          onPointerDown={startResize}
          onPointerMove={resize}
          onPointerUp={() => {
            drag.current = undefined;
          }}
        />
      )}
    </div>
  );
}
