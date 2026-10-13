import { type ReactNode, useRef } from "react";
import { Icon, type IconName } from "../ui/icons";

export interface SideState {
  open: boolean;
  width: number;
  view: string;
}

export interface SideView {
  id: string;
  title: string;
  icon: IconName;
  /** A count on the tab, such as pending comments. Hidden at 0. */
  badge?: number;
  badgeTone?: "amber";
  render(): ReactNode;
}

interface Props {
  side: "left" | "right";
  views: SideView[];
  state: SideState;
  onChange(next: SideState): void;
  /** Set to focus the panel's content, e.g. after a shortcut opens it. */
  bodyRef?: React.RefObject<HTMLDivElement | null>;
}

const MIN_WIDTH = 160;
const MAX_WIDTH = 600;

/** A panel docked to one side of the window, outside the pane grid. Holds views behind a row
 * of icon tabs, shown when there's more than one, and resizes by dragging its inner edge. Each
 * view draws its own header. */
export function SidePanel({ side, views, state, onChange, bodyRef }: Props) {
  const drag = useRef<{ x: number; width: number }>(undefined);
  if (!state.open) return null;
  const view = views.find((v) => v.id === state.view) ?? views[0];

  function startResize(e: React.PointerEvent) {
    e.preventDefault();
    drag.current = { x: e.clientX, width: state.width };
    (e.target as Element).setPointerCapture(e.pointerId);
  }

  function resize(e: React.PointerEvent) {
    if (!drag.current) return;
    const dx = e.clientX - drag.current.x;
    const width = drag.current.width + (side === "left" ? dx : -dx);
    onChange({ ...state, width: Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, width)) });
  }

  return (
    <aside className={`side-panel side-${side}`} style={{ width: state.width }}>
      {views.length > 1 && (
        <div className="side-tabs" role="tablist">
          {views.map((v) => (
            <button
              type="button"
              role="tab"
              key={v.id}
              aria-selected={v.id === view.id}
              aria-label={v.title}
              title={v.title}
              className={v.id === view.id ? "selected" : undefined}
              onClick={() => onChange({ ...state, view: v.id })}
            >
              <Icon name={v.icon} />
              {!!v.badge && (
                <span className={`side-badge${v.badgeTone ? ` ${v.badgeTone}` : ""}`}>
                  {v.badge}
                </span>
              )}
            </button>
          ))}
        </div>
      )}
      <div className="side-body" ref={bodyRef} tabIndex={-1}>
        {view.render()}
      </div>
      <div
        className="side-resizer"
        onPointerDown={startResize}
        onPointerMove={resize}
        onPointerUp={() => {
          drag.current = undefined;
        }}
      />
    </aside>
  );
}
