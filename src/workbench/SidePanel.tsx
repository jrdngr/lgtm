import { Fragment, type ReactNode, useRef } from "react";
import { contextMenu } from "../ui/contextMenu";
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
  /** Views of one group sit together, with a divider between groups. */
  group?: string;
  /** Tints the tab's icon amber, for something waiting on you. */
  alert?: boolean;
  render(): ReactNode;
}

interface Props {
  side: "left" | "right";
  views: SideView[];
  state: SideState;
  onChange(next: SideState): void;
  /** Set to focus the panel's content, e.g. after a shortcut opens it. */
  bodyRef?: React.RefObject<HTMLDivElement | null>;
  /** Hides the panel, from the icon tabs' menu. */
  onHide?(): void;
}

const MIN_WIDTH = 160;
const MAX_WIDTH = 600;

/** A panel docked to one side of the window, outside the pane grid. Holds views behind a row
 * of icon tabs, shown when there's more than one and divided by the views' groups, and resizes
 * by dragging its inner edge. Each view draws its own header. */
export function SidePanel({ side, views, state, onChange, bodyRef, onHide }: Props) {
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
        <div
          className="side-tabs"
          role="tablist"
          onContextMenu={contextMenu(() => [
            onHide && {
              label: side === "left" ? "Hide Left Panel" : "Hide Right Panel",
              keys: side === "left" ? "⌘B" : "⌥⌘B",
              onSelect: onHide,
            },
          ])}
        >
          {views.map((v, i) => (
            <Fragment key={v.id}>
              {i > 0 && v.group !== views[i - 1].group && <span className="side-divider" />}
              <button
                type="button"
                role="tab"
                aria-selected={v.id === view.id}
                aria-label={v.title}
                title={v.title}
                className={
                  [v.id === view.id && "selected", v.alert && "alert"].filter(Boolean).join(" ") ||
                  undefined
                }
                onClick={() => onChange({ ...state, view: v.id })}
              >
                <Icon name={v.icon} />
              </button>
            </Fragment>
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
