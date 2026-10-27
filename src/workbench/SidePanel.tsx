import { Fragment, type ReactNode, useRef } from "react";
import { contextMenu } from "../ui/contextMenu";
import { Icon, type IconName } from "../ui/icons";

export interface SideState {
  open: boolean;
  width: number;
  view: string;
  /** A second view below the first, kept while closed so it comes back as it was. */
  split?: SideSplit;
}

export interface SideSplit {
  open: boolean;
  view: string;
  /** The top view's share of the height. */
  ratio: number;
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
  /** Offers a second view below the first, from a button at the bottom of the tabs. */
  splittable?: boolean;
}

const MIN_WIDTH = 160;
const MAX_WIDTH = 600;
const MIN_RATIO = 0.15;

/** A panel docked to one side of the window, outside the pane grid. Holds views behind a strip
 * of icon tabs down its outer edge, shown when there's more than one and divided by the views'
 * groups, and resizes by dragging its inner edge. Each view draws its own header. Hidden, the
 * strip stays: a tab opens the panel on its view, and the open view's tab hides it. Split, a
 * second view with its own tabs sits below the first, and the line between them drags. */
export function SidePanel({ side, views, state, onChange, bodyRef, onHide, splittable }: Props) {
  const drag = useRef<{ x: number; width: number }>(undefined);
  const aside = useRef<HTMLElement>(null);
  const splitDrag = useRef(false);
  const tabbed = views.length > 1;
  if (!state.open && !tabbed) return null;
  const find = (id: string) => views.find((v) => v.id === id) ?? views[0];
  const view = find(state.view);
  const split = state.open && state.split?.open ? state.split : undefined;
  const lower = split && find(split.view);
  const pick = (id: string) =>
    onChange(
      state.open && id === view.id ? { ...state, open: false } : { ...state, open: true, view: id },
    );
  const pickLower = (id: string) => split && onChange({ ...state, split: { ...split, view: id } });
  const toggleSplit = () => {
    if (split) return onChange({ ...state, split: { ...split, open: false } });
    const last = state.split;
    const other = views.find((v) => v.id !== view.id)?.id ?? view.id;
    const below = last && last.view !== view.id ? last.view : other;
    onChange({ ...state, split: { open: true, view: below, ratio: last?.ratio ?? 0.5 } });
  };

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

  function resizeSplit(e: React.PointerEvent) {
    const box = aside.current?.getBoundingClientRect();
    if (!splitDrag.current || !split || !box) return;
    const ratio = (e.clientY - box.top) / box.height;
    const clamped = Math.min(1 - MIN_RATIO, Math.max(MIN_RATIO, ratio));
    onChange({ ...state, split: { ...split, ratio: clamped } });
  }

  const menu = contextMenu(() => [
    onHide && {
      label: side === "left" ? "Hide Left Panel" : "Hide Right Panel",
      keys: side === "left" ? "⌘B" : "⌥⌘B",
      onSelect: onHide,
    },
  ]);
  const splitButton = splittable && state.open && (
    <button
      type="button"
      className={`side-split-toggle${split ? " selected" : ""}`}
      aria-pressed={!!split}
      aria-label="Split"
      title="Split"
      onClick={toggleSplit}
    >
      <Icon name="stack" />
    </button>
  );

  return (
    <aside
      ref={aside}
      className={`side-panel side-${side}${state.open ? "" : " shut"}`}
      style={state.open ? { width: state.width } : undefined}
    >
      <div className="side-half" style={split ? { flex: split.ratio } : undefined}>
        {tabbed && (
          <SideTabs
            views={views}
            selected={state.open ? view.id : undefined}
            onPick={pick}
            onContextMenu={menu}
            end={!split && splitButton}
          />
        )}
        {state.open && (
          <div className="side-body" ref={bodyRef} tabIndex={-1}>
            {view.render()}
          </div>
        )}
      </div>
      {split && lower && (
        <>
          <div
            className="side-split-resizer"
            onPointerDown={(e) => {
              e.preventDefault();
              splitDrag.current = true;
              (e.target as Element).setPointerCapture(e.pointerId);
            }}
            onPointerMove={resizeSplit}
            onPointerUp={() => {
              splitDrag.current = false;
            }}
          />
          <div className="side-half" style={{ flex: 1 - split.ratio }}>
            <SideTabs
              views={views}
              selected={lower.id}
              onPick={pickLower}
              onContextMenu={menu}
              end={splitButton}
            />
            <div className="side-body" tabIndex={-1}>
              {lower.render()}
            </div>
          </div>
        </>
      )}
      {state.open && (
        <div
          className="side-resizer"
          onPointerDown={startResize}
          onPointerMove={resize}
          onPointerUp={() => {
            drag.current = undefined;
          }}
        />
      )}
    </aside>
  );
}

interface TabsProps {
  views: SideView[];
  selected: string | undefined;
  onPick(id: string): void;
  onContextMenu(e: React.MouseEvent): void;
  /** Sits at the bottom of the strip. */
  end?: ReactNode;
}

function SideTabs({ views, selected, onPick, onContextMenu, end }: TabsProps) {
  return (
    <div className="side-tabs" role="tablist" onContextMenu={onContextMenu}>
      {views.map((v, i) => (
        <Fragment key={v.id}>
          {i > 0 && v.group !== views[i - 1].group && <span className="side-divider" />}
          <button
            type="button"
            role="tab"
            aria-selected={v.id === selected}
            aria-label={v.title}
            title={v.title}
            className={
              [v.id === selected && "selected", v.alert && "alert"].filter(Boolean).join(" ") ||
              undefined
            }
            onClick={() => onPick(v.id)}
          >
            <Icon name={v.icon} />
          </button>
        </Fragment>
      ))}
      {end}
    </div>
  );
}
