import type { IDockviewHeaderActionsProps } from "dockview-react";
import { useState } from "react";
import { createPortal } from "react-dom";
import { addToGroup, KINDS, kindTitle, splitGroup } from "./panels";
import "../review/MarkMenu.css";
import "./GroupActions.css";

/** A tab after a group's tabs that opens a menu of the kinds of tab to add there. */
export function NewTab({ containerApi, group }: IDockviewHeaderActionsProps) {
  const [at, setAt] = useState<{ x: number; y: number }>();
  const close = () => setAt(undefined);
  return (
    <>
      <button
        type="button"
        className="new-tab"
        title="New tab"
        aria-label="New tab"
        onClick={(e) => {
          const r = e.currentTarget.getBoundingClientRect();
          setAt({ x: r.left, y: r.bottom });
        }}
      >
        +
      </button>
      {at &&
        // In the body, so the group's header doesn't clip it.
        createPortal(
          <>
            <div className="menu-backdrop new-tab-backdrop" onMouseDown={close} />
            <div className="mark-menu new-tab-menu" style={{ left: at.x, top: at.y }}>
              {KINDS.map((kind) => (
                <button
                  type="button"
                  key={kind}
                  onClick={() => {
                    close();
                    addToGroup(containerApi, group, kind);
                  }}
                >
                  {kindTitle(kind)}
                </button>
              ))}
            </div>
          </>,
          document.body,
        )}
    </>
  );
}

/** Two rects side by side, or stacked. */
function SplitIcon({ stacked }: { stacked: boolean }) {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true">
      <rect x="1.5" y="2.5" width="13" height="11" rx="1" fill="none" stroke="currentColor" />
      {stacked ? (
        <line x1="1.5" y1="8" x2="14.5" y2="8" stroke="currentColor" />
      ) : (
        <line x1="8" y1="2.5" x2="8" y2="13.5" stroke="currentColor" />
      )}
    </svg>
  );
}

/** Buttons that split the group, right or down, with an empty pane. */
export function SplitButtons({ containerApi, group }: IDockviewHeaderActionsProps) {
  return (
    <div className="split-buttons">
      <button
        type="button"
        title="Split right"
        aria-label="Split right"
        onClick={() => splitGroup(containerApi, group, "right")}
      >
        <SplitIcon stacked={false} />
      </button>
      <button
        type="button"
        title="Split down"
        aria-label="Split down"
        onClick={() => splitGroup(containerApi, group, "below")}
      >
        <SplitIcon stacked />
      </button>
    </div>
  );
}
