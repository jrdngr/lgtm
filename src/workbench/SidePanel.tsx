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

/** A panel docked to one side of the window, outside the pane grid. Holds views behind a strip
 * of icon tabs down its outer edge, shown when there's more than one and divided by the views'
 * groups, and resizes by dragging its inner edge. Each view draws its own header. Hidden, the
 * strip stays: a tab opens the panel on its view, and the open view's tab hides it. */
export function SidePanel({ side, views, state, onChange, bodyRef, onHide }: Props) {
  const drag = useRef<{ x: number; width: number }>(undefined);
  const tabbed = views.length > 1;
  if (!state.open && !tabbed) return null;
  const view = views.find((v) => v.id === state.view) ?? views[0];
  const pick = (id: string) =>
    onChange(
      state.open && id === view.id ? { ...state, open: false } : { ...state, open: true, view: id },
    );

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
    <aside
      className={`side-panel side-${side}${state.open ? "" : " shut"}`}
      style={state.open ? { width: state.width } : undefined}
    >
      {tabbed && (
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
                aria-selected={state.open && v.id === view.id}
                aria-label={v.title}
                title={v.title}
                className={
                  [state.open && v.id === view.id && "selected", v.alert && "alert"]
                    .filter(Boolean)
                    .join(" ") || undefined
                }
                onClick={() => pick(v.id)}
              >
                <Icon name={v.icon} />
              </button>
            </Fragment>
          ))}
        </div>
      )}
      {state.open && (
        <>
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
        </>
      )}
    </aside>
  );
}
